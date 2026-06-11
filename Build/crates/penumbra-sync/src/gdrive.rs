use std::collections::HashMap;

use async_trait::async_trait;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::{PenumbraError, Result};

use crate::provider::{SyncProvider, SyncPullResult, SyncStatus};
use crate::snapshot::SyncSnapshot;

/// Google Drive sync backend.
///
/// Syncs notes as Drive files (one JSON file per note).
/// **Incomplete**: this is a stub that will be implemented in a later commit.
///
pub struct GoogleDriveSyncProvider {
    _client_id: String,
}

impl GoogleDriveSyncProvider {
    pub fn new(client_id: &str) -> Self {
        Self {
            _client_id: client_id.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl SyncProvider for GoogleDriveSyncProvider {
    async fn connect(&mut self) -> Result<()> {
        Err(PenumbraError::Sync(
            "GoogleDriveSyncProvider is not yet implemented".into(),
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
            "GoogleDriveSyncProvider: push not implemented".into(),
        ))
    }

    async fn pull(&self, _since_snapshot: Option<&str>) -> Result<SyncPullResult> {
        Err(PenumbraError::Sync(
            "GoogleDriveSyncProvider: pull not implemented".into(),
        ))
    }

    async fn status(&self) -> Result<SyncStatus> {
        Err(PenumbraError::Sync(
            "GoogleDriveSyncProvider: status not implemented".into(),
        ))
    }
}
