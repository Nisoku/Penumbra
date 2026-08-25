use std::collections::HashMap;

use futures::StreamExt;
use opfs::persistent::{app_specific_dir, DirectoryHandle, FileHandle};
use opfs::{
    CreateWritableOptions, DirectoryHandle as _, FileHandle as _, GetDirectoryHandleOptions,
    GetFileHandleOptions, WritableFileStream as _,
};
use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::link::Link;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;

const APP_DIR: &str = "Penumbra";
const NOTES_DIR: &str = "notes";
const GRAPH_FILE: &str = "graph.json";
const POSITIONS_FILE: &str = "positions.json";

pub struct Storage {
    root: DirectoryHandle,
}

impl Storage {
    pub async fn new() -> Result<Self> {
        let data_dir = app_specific_dir()
            .await
            .map_err(|e| PenumbraError::Storage(format!("app dir: {e:?}")))?;
        // The platform data dir is shared with other apps, so everything
        // lives under a namespaced subdirectory on every target.
        let root = data_dir
            .get_directory_handle_with_options(APP_DIR, &GetDirectoryHandleOptions { create: true })
            .await
            .map_err(|e| PenumbraError::Storage(format!("app subdir: {e:?}")))?;
        Ok(Self { root })
    }

    pub async fn with_dir(root: DirectoryHandle) -> Self {
        Self { root }
    }

    async fn notes_dir(&self) -> Result<DirectoryHandle> {
        self.root
            .get_directory_handle_with_options(
                NOTES_DIR,
                &GetDirectoryHandleOptions { create: true },
            )
            .await
            .map_err(|e| PenumbraError::Storage(format!("notes dir: {e:?}")))
    }

    async fn note_file(&self, id: &NoteId) -> Result<FileHandle> {
        let dir = self.notes_dir().await?;
        let filename = format!("{}.json", id);
        dir.get_file_handle_with_options(&filename, &GetFileHandleOptions { create: true })
            .await
            .map_err(|e| PenumbraError::Storage(format!("note file: {e:?}")))
    }

    async fn write_json(&self, file: &mut FileHandle, json: String) -> Result<()> {
        let mut writer = file
            .create_writable_with_options(&CreateWritableOptions {
                keep_existing_data: false,
            })
            .await
            .map_err(|e| PenumbraError::Storage(format!("create writable: {e:?}")))?;

        let bytes = json.into_bytes();
        writer
            .write_at_cursor_pos(&bytes)
            .await
            .map_err(|e| PenumbraError::Storage(format!("write: {e:?}")))?;
        writer
            .close()
            .await
            .map_err(|e| PenumbraError::Storage(format!("close: {e:?}")))?;
        Ok(())
    }

    async fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        file: &FileHandle,
    ) -> Result<Option<T>> {
        let data = file
            .read()
            .await
            .map_err(|e| PenumbraError::Storage(format!("read: {e:?}")))?;
        if data.is_empty() {
            return Ok(None);
        }
        let value: T = serde_json::from_slice(&data)?;
        Ok(Some(value))
    }

    async fn root_file(&self, name: &str, create: bool) -> Result<FileHandle> {
        self.root
            .get_file_handle_with_options(name, &GetFileHandleOptions { create })
            .await
            .map_err(|e| PenumbraError::Storage(format!("file {name}: {e:?}")))
    }

    // Notes

    pub async fn save_note(&self, note: &Note) -> Result<()> {
        let mut file = self.note_file(&note.id).await?;
        let json = serde_json::to_string_pretty(note)?;
        self.write_json(&mut file, json).await
    }

    pub async fn load_note(&self, id: &NoteId) -> Result<Option<Note>> {
        let file = match self.note_file(id).await {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        self.read_json(&file).await
    }

    pub async fn delete_note(&self, id: &NoteId) -> Result<()> {
        let mut dir = self.notes_dir().await?;
        let filename = format!("{}.json", id);
        dir.remove_entry(&filename)
            .await
            .map_err(|e| PenumbraError::Storage(format!("remove: {e:?}")))?;
        Ok(())
    }

    pub async fn load_notes_batch(&self, ids: &[NoteId]) -> Result<Vec<Note>> {
        let mut notes = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(note) = self.load_note(id).await? {
                notes.push(note);
            }
        }
        Ok(notes)
    }

    // Graph

    pub async fn save_graph(&self, notes: &[Note], links: &[Link]) -> Result<()> {
        #[derive(serde::Serialize)]
        struct GraphData<'a> {
            notes: &'a [Note],
            links: &'a [Link],
        }
        let mut file = self.root_file(GRAPH_FILE, true).await?;
        let json = serde_json::to_string_pretty(&GraphData { notes, links })?;
        self.write_json(&mut file, json).await
    }

    pub async fn load_graph(&self) -> Result<Option<(Vec<Note>, Vec<Link>)>> {
        let file = match self.root_file(GRAPH_FILE, false).await {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        #[derive(serde::Deserialize)]
        struct GraphData {
            notes: Vec<Note>,
            links: Vec<Link>,
        }
        let parsed: Option<GraphData> = self.read_json(&file).await?;
        Ok(parsed.map(|d| (d.notes, d.links)))
    }

    // Positions

    pub async fn save_positions(&self, positions: &HashMap<NoteId, Position>) -> Result<()> {
        let mut file = self.root_file(POSITIONS_FILE, true).await?;
        let json = serde_json::to_string_pretty(positions)?;
        self.write_json(&mut file, json).await
    }

    pub async fn load_positions(&self) -> Result<Option<HashMap<NoteId, Position>>> {
        let file = match self.root_file(POSITIONS_FILE, false).await {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        self.read_json(&file).await
    }

    // Index / cache

    /// List all note IDs from the notes directory.
    pub async fn list_note_ids(&self) -> Result<Vec<NoteId>> {
        let dir = match self.notes_dir().await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let mut stream = dir
            .entries()
            .await
            .map_err(|e| PenumbraError::Storage(format!("entries: {e:?}")))?;

        let mut ids = Vec::new();
        while let Some(entry) = stream.next().await {
            let (name, _kind) =
                entry.map_err(|e| PenumbraError::Storage(format!("entry: {e:?}")))?;
            if let Some(stripped) = name.strip_suffix(".json") {
                if let Ok(uuid) = uuid::Uuid::parse_str(stripped) {
                    ids.push(NoteId::from_raw(uuid));
                }
            }
        }
        Ok(ids)
    }
}
