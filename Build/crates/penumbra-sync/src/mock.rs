use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::Result;

use penumbra_core::PenumbraError;

use crate::provider::{SyncProvider, SyncPullResult, SyncStatus};
use crate::snapshot::SyncSnapshot;

/// In-memory sync provider for testing.
///
/// Stores everything locally so tests can verify push/pull behaviour
/// without a real network.
pub struct MockSyncProvider {
    notes: Mutex<HashMap<NoteId, Note>>,
    embeddings: Mutex<HashMap<NoteId, Vec<f32>>>,
    positions: Mutex<HashMap<NoteId, Position>>,
    snapshot: Mutex<Option<SyncSnapshot>>,
    last_sync: Mutex<Option<DateTime<Utc>>>,
    fail_on_command: Mutex<bool>,
}

impl MockSyncProvider {
    pub fn new() -> Self {
        Self {
            notes: Mutex::new(HashMap::new()),
            embeddings: Mutex::new(HashMap::new()),
            positions: Mutex::new(HashMap::new()),
            snapshot: Mutex::new(None),
            last_sync: Mutex::new(None),
            fail_on_command: Mutex::new(false),
        }
    }

    /// When set to true, all operations return an error.
    pub fn set_fail(&self, fail: bool) {
        *self.fail_on_command.lock().unwrap() = fail;
    }

    /// Inspect stored notes (for test assertions).
    pub fn stored_notes(&self) -> Vec<Note> {
        self.notes.lock().unwrap().values().cloned().collect()
    }
}

impl Default for MockSyncProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl SyncProvider for MockSyncProvider {
    async fn connect(&mut self) -> Result<()> {
        if *self.fail_on_command.lock().unwrap() {
            Err(PenumbraError::Sync("mock connect failed".into()))
        } else {
            Ok(())
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn push(
        &self,
        notes: &[Note],
        embeddings: &HashMap<NoteId, Vec<f32>>,
        positions: &HashMap<NoteId, Position>,
        _snapshot_id: Option<&str>,
    ) -> Result<SyncSnapshot> {
        if *self.fail_on_command.lock().unwrap() {
            return Err(PenumbraError::Sync("mock push failed".into()));
        }

        {
            let mut n = self.notes.lock().unwrap();
            for note in notes {
                n.insert(note.id, note.clone());
            }
        }
        {
            let mut e = self.embeddings.lock().unwrap();
            for (id, vec) in embeddings {
                e.insert(*id, vec.clone());
            }
        }
        {
            let mut p = self.positions.lock().unwrap();
            for (id, pos) in positions {
                p.insert(*id, *pos);
            }
        }

        let snapshot = SyncSnapshot::new(notes.iter().map(|n| n.id.to_string()).collect());
        *self.snapshot.lock().unwrap() = Some(snapshot.clone());
        *self.last_sync.lock().unwrap() = Some(Utc::now());

        Ok(snapshot)
    }

    async fn pull(&self, _since_snapshot: Option<&str>) -> Result<SyncPullResult> {
        if *self.fail_on_command.lock().unwrap() {
            return Err(PenumbraError::Sync("mock pull failed".into()));
        }

        let notes = self.notes.lock().unwrap().clone();
        let embeddings = self.embeddings.lock().unwrap().clone();
        let positions = self.positions.lock().unwrap().clone();
        let snapshot = self.snapshot.lock().unwrap().clone();

        Ok(SyncPullResult {
            notes,
            embeddings,
            positions,
            snapshot,
        })
    }

    async fn status(&self) -> Result<SyncStatus> {
        if *self.fail_on_command.lock().unwrap() {
            return Err(PenumbraError::Sync("mock status failed".into()));
        }

        let note_count = self.notes.lock().unwrap().len() as u64;
        let last_sync = *self.last_sync.lock().unwrap();
        let snapshot_id = self
            .snapshot
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.snapshot_id.clone());

        Ok(SyncStatus {
            note_count,
            last_modified: last_sync,
            storage_bytes: 0,
            storage_limit: 512 * 1024 * 1024,
            snapshot_id,
        })
    }

    fn last_sync(&self) -> Option<DateTime<Utc>> {
        *self.last_sync.lock().unwrap()
    }
}
