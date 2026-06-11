use std::collections::HashMap;

use async_trait::async_trait;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::{PenumbraError, Result};

use crate::provider::{SyncProvider, SyncPullResult, SyncStatus};
use crate::snapshot::SyncSnapshot;

/// Git-based sync backend.
///
/// Uses a remote git repository as a snapshot store.  Each sync creates a
/// commit with the full note set.  
/// **Incomplete**: this is a stub that will be implemented in a follow-up.
///
pub struct GitSyncProvider {
    _repo_path: String,
    _remote_url: String,
}

impl GitSyncProvider {
    pub fn new(repo_path: &str, remote_url: &str) -> Self {
        Self {
            _repo_path: repo_path.to_string(),
            _remote_url: remote_url.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl SyncProvider for GitSyncProvider {
    async fn connect(&mut self) -> Result<()> {
        Err(PenumbraError::Sync(
            "GitSyncProvider is not yet implemented".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn push(
        &self,
        _notes: &[Note],
        _embeddings: &HashMap<NoteId, Vec<f32>>,
        _positions: &HashMap<NoteId, Position>,
        _snapshot_id: Option<&str>,
    ) -> Result<SyncSnapshot> {
        Err(PenumbraError::Sync(
            "GitSyncProvider: push not implemented".into(),
        ))
    }

    async fn pull(&self, _since_snapshot: Option<&str>) -> Result<SyncPullResult> {
        Err(PenumbraError::Sync(
            "GitSyncProvider: pull not implemented".into(),
        ))
    }

    async fn status(&self) -> Result<SyncStatus> {
        Err(PenumbraError::Sync(
            "GitSyncProvider: status not implemented".into(),
        ))
    }
}
