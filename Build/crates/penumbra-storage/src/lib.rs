//! Penumbra storage crate.

mod filename;

use std::collections::HashMap;
use std::sync::Mutex;

use futures::StreamExt;
use opfs::persistent::{DirectoryHandle, FileHandle};
use opfs::{
    CreateWritableOptions, DirectoryHandle as _, FileHandle as _, GetDirectoryHandleOptions,
    GetFileHandleOptions, WritableFileStream as _,
};
use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_markdown::frontmatter::{self, Frontmatter};
use penumbra_markdown::links::{extract_inline_tags, extract_wikilinks};
use penumbra_markdown::parser::parse_document;

const APP_SUBDIR: &str = "Penumbra";
const STATE_DIR: &str = ".penumbra";
const WORKSPACE_FILE: &str = "workspace.json";
const NOTE_EXTENSION: &str = ".md";

/// A note as loaded from the vault, with the bookkeeping needed to write
/// it back faithfully.
#[derive(Debug, Clone)]
pub struct StoredNote {
    pub note: Note,
    /// File stem the note was loaded from (the title at scan time).
    pub filename: String,
    /// Tags that came from frontmatter only; inline body tags must not
    /// migrate into YAML just because Penumbra saved the file.
    pub fm_tags: Vec<String>,
}

#[derive(Default)]
struct VaultIndex {
    by_id: HashMap<NoteId, String>,
    by_title: HashMap<String, NoteId>,
}

pub struct Storage {
    root: DirectoryHandle,
    index: Mutex<VaultIndex>,
}

impl Storage {
    /// Default vault root for this platform, created if missing.
    pub async fn platform_default_root() -> Result<DirectoryHandle> {
        #[cfg(target_arch = "wasm32")]
        {
            let base = opfs::persistent::app_specific_dir()
                .await
                .map_err(|e| PenumbraError::Storage(format!("app dir: {e:?}")))?;
            base.get_directory_handle_with_options(
                APP_SUBDIR,
                &GetDirectoryHandleOptions { create: true },
            )
            .await
            .map_err(|e| PenumbraError::Storage(format!("vault dir: {e:?}")))
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let base = dirs::document_dir()
                .or_else(dirs::home_dir)
                .ok_or_else(|| {
                    PenumbraError::Storage("no documents or home directory".to_string())
                })?;
            let vault = base.join(APP_SUBDIR);
            std::fs::create_dir_all(&vault)?;
            Ok(DirectoryHandle::from(vault))
        }
    }

    /// Open the vault at the platform default location.
    pub async fn new() -> Result<Self> {
        let root = Self::platform_default_root().await?;
        Ok(Self::with_dir(root).await)
    }

    /// Wrap an externally chosen directory (picker result) as a vault.
    ///
    /// The caller should run [`Storage::scan`] before mutating operations
    /// so rename dedupe knows about existing files.
    pub async fn with_dir(root: DirectoryHandle) -> Self {
        Self {
            root,
            index: Mutex::new(VaultIndex::default()),
        }
    }

    /// Read every note file in the vault and rebuild the filename index.
    ///
    /// Files with malformed frontmatter are skipped with a warning rather
    /// than failing the whole scan; plain markdown without frontmatter is
    /// adopted with a freshly generated identity.
    pub async fn scan(&self) -> Result<Vec<StoredNote>> {
        let mut stream = self
            .root
            .entries()
            .await
            .map_err(|e| PenumbraError::Storage(format!("vault entries: {e:?}")))?;

        let mut stored = Vec::new();
        while let Some(entry) = stream.next().await {
            let (name, kind) =
                entry.map_err(|e| PenumbraError::Storage(format!("entry: {e:?}")))?;
            if !matches!(kind, opfs::DirectoryEntry::File(_)) {
                continue;
            }
            let Some(stem) = name.strip_suffix(NOTE_EXTENSION) else {
                continue;
            };
            match self.read_note_file(stem).await {
                Ok(item) => stored.push(item),
                Err(err) => tracing::warn!("skipping {name}: {err}"),
            }
        }

        stored.sort_by(|a, b| a.note.title.cmp(&b.note.title));
        let mut index = self.index.lock().expect("vault index poisoned");
        index.by_id.clear();
        index.by_title.clear();
        for item in &stored {
            if index.by_id.contains_key(&item.note.id) {
                tracing::warn!(
                    "duplicate id {} across note files, keeping first seen",
                    item.note.id
                );
                continue;
            }
            index.by_id.insert(item.note.id, item.filename.clone());
            index
                .by_title
                .insert(item.filename.to_lowercase(), item.note.id);
        }
        Ok(stored)
    }

    /// Persist a note under its title-derived filename, renaming when the
    /// title changed since load. Returns the final file stem written.
    pub async fn save_note(&self, note: &Note, structured_tags: &[String]) -> Result<String> {
        let desired = filename::sanitize_title_to_stem(&note.title);

        // decide the target name without mutating state and with
        // no lock held across await points.
        enum Plan {
            Keep(String),
            Write { old: Option<String>, new: String },
        }
        let plan = {
            let index = self.index.lock().expect("vault index poisoned");
            match index.by_id.get(&note.id).cloned() {
                Some(current) if current.eq_ignore_ascii_case(&desired) => Plan::Keep(current),
                current => {
                    let candidate = filename::dedupe_stem(desired.clone(), &mut |stem| {
                        index
                            .by_title
                            .get(stem.to_lowercase().as_str())
                            .is_some_and(|owner| owner != &note.id)
                    });
                    Plan::Write {
                        old: current,
                        new: candidate,
                    }
                }
            }
        };

        // file operations. Files are the source of truth, so any
        // crash mid-sequence is repaired by the next scan.
        let final_stem = match plan {
            Plan::Keep(stem) => {
                self.write_note_file(&stem, note, structured_tags).await?;
                stem
            }
            Plan::Write { old, new } => {
                self.write_note_file(&new, note, structured_tags).await?;
                if let Some(old) = &old {
                    self.delete_file(old).await?;
                }
                let mut index = self.index.lock().expect("vault index poisoned");
                if let Some(old) = &old {
                    index.by_title.remove(&old.to_lowercase());
                }
                index.by_title.insert(new.to_lowercase(), note.id);
                index.by_id.insert(note.id, new.clone());
                new
            }
        };
        Ok(final_stem)
    }

    async fn write_note_file(
        &self,
        stem: &str,
        note: &Note,
        structured_tags: &[String],
    ) -> Result<()> {
        let content = render_note_file(note, structured_tags)?;
        let mut file = self
            .root
            .get_file_handle_with_options(
                &format!("{stem}{NOTE_EXTENSION}"),
                &GetFileHandleOptions { create: true },
            )
            .await
            .map_err(|e| PenumbraError::Storage(format!("note file: {e:?}")))?;
        self.write_all(&mut file, content.into_bytes()).await
    }

    /// Remove a note's file and forget its identity. Deleting a file that
    /// vanished externally still succeeds.
    pub async fn delete_note(&self, id: &NoteId) -> Result<()> {
        let stem = {
            let mut index = self.index.lock().expect("vault index poisoned");
            match index.by_id.remove(id) {
                Some(stem) => {
                    index.by_title.remove(&stem.to_lowercase());
                    Some(stem)
                }
                None => None,
            }
        };
        if let Some(stem) = stem {
            self.delete_file(&stem).await?;
        }
        Ok(())
    }

    /// Current filename registered for a note, if scanned.
    pub fn filename_of(&self, id: &NoteId) -> Option<String> {
        self.index
            .lock()
            .expect("vault index poisoned")
            .by_id
            .get(id)
            .cloned()
    }

    // Workspace state (.penumbra/workspace.json)

    pub async fn save_positions(&self, positions: &HashMap<NoteId, Position>) -> Result<()> {
        let mut file = self.workspace_file(true).await?;
        let json = serde_json::to_string_pretty(positions)?;
        self.write_all(&mut file, json.into_bytes()).await
    }

    pub async fn load_positions(&self) -> Result<Option<HashMap<NoteId, Position>>> {
        let file = match self.workspace_file(false).await {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        let data = self.read_all(&file).await?;
        if data.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&data)?))
    }

    // Internals

    async fn read_note_file(&self, stem: &str) -> Result<StoredNote> {
        let file = self
            .root
            .get_file_handle_with_options(
                &format!("{stem}{NOTE_EXTENSION}"),
                &GetFileHandleOptions { create: false },
            )
            .await
            .map_err(|e| PenumbraError::Storage(format!("open {stem}: {e:?}")))?;
        let bytes = self.read_all(&file).await?;
        let text = String::from_utf8(bytes)
            .map_err(|e| PenumbraError::Markdown(format!("{stem}: invalid utf-8: {e}")))?;

        let parsed = frontmatter::parse(&text)?;
        let doc = parse_document(&parsed.body)?;
        let inline_tags = extract_inline_tags(&doc);

        let now = chrono::Utc::now();
        let (raw_id, created_at, updated_at, pinned, archived, fm_tags) = match parsed.frontmatter {
            Some(fm) => (
                fm.id,
                fm.created_at,
                fm.updated_at,
                fm.pinned,
                fm.archived,
                fm.tags,
            ),
            None => (uuid::Uuid::new_v4(), now, now, false, false, Vec::new()),
        };
        let id = NoteId::from_raw(raw_id);

        let mut tags = fm_tags.clone();
        for tag in inline_tags {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }

        Ok(StoredNote {
            note: Note {
                id,
                title: stem.to_string(),
                body: parsed.body,
                tags,
                meta: penumbra_core::note::NoteMeta {
                    created_at,
                    updated_at,
                    pinned,
                    archived,
                },
            },
            filename: stem.to_string(),
            fm_tags,
        })
    }

    async fn workspace_file(&self, create: bool) -> Result<FileHandle> {
        let dir = self
            .root
            .get_directory_handle_with_options(STATE_DIR, &GetDirectoryHandleOptions { create })
            .await
            .map_err(|e| PenumbraError::Storage(format!("state dir: {e:?}")))?;
        dir.get_file_handle_with_options(WORKSPACE_FILE, &GetFileHandleOptions { create })
            .await
            .map_err(|e| PenumbraError::Storage(format!("workspace file: {e:?}")))
    }

    async fn delete_file(&self, stem: &str) -> Result<()> {
        let name = format!("{stem}{NOTE_EXTENSION}");
        // Existence probe first: error shapes differ per platform, so
        // matching on NotFound from remove_entry is not portable.
        if self
            .root
            .get_file_handle_with_options(&name, &GetFileHandleOptions { create: false })
            .await
            .is_err()
        {
            return Ok(());
        }
        let mut root = self.root.clone();
        root.remove_entry(&name)
            .await
            .map_err(|e| PenumbraError::Storage(format!("remove {stem}: {e:?}")))?;
        Ok(())
    }

    async fn write_all(&self, file: &mut FileHandle, bytes: Vec<u8>) -> Result<()> {
        let mut writer = file
            .create_writable_with_options(&CreateWritableOptions {
                keep_existing_data: false,
            })
            .await
            .map_err(|e| PenumbraError::Storage(format!("create writable: {e:?}")))?;
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

    async fn read_all(&self, file: &FileHandle) -> Result<Vec<u8>> {
        file.read()
            .await
            .map_err(|e| PenumbraError::Storage(format!("read: {e:?}")))
    }
}

fn render_note_file(note: &Note, structured_tags: &[String]) -> Result<String> {
    let fm = Frontmatter {
        id: *note.id.as_uuid(),
        created_at: note.meta.created_at,
        updated_at: note.meta.updated_at,
        tags: structured_tags.to_vec(),
        pinned: note.meta.pinned,
        archived: note.meta.archived,
    };
    let mut out = frontmatter::serialize(&fm)?;
    out.push_str(&note.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Wikilink targets referenced by a note body, resolved-ready titles.
pub fn wikilink_targets(body: &str) -> Vec<String> {
    match parse_document(body) {
        Ok(doc) => extract_wikilinks(&doc),
        Err(_) => Vec::new(),
    }
}
