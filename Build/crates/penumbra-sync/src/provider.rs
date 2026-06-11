use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::Result;

use crate::snapshot::SyncSnapshot;

/// Result of a pull operation.
#[derive(Debug, Clone)]
pub struct SyncPullResult {
    pub notes: HashMap<NoteId, Note>,
    pub embeddings: HashMap<NoteId, Vec<f32>>,
    pub positions: HashMap<NoteId, Position>,
    pub snapshot: Option<SyncSnapshot>,
}

/// Storage status from the remote.
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub note_count: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub storage_bytes: u64,
    pub storage_limit: u64,
    pub snapshot_id: Option<String>,
}

/// Pluggable sync backend.
///
/// Each implementation connects Penumbra to a different cloud provider.
#[async_trait(?Send)]
pub trait SyncProvider: Send + Sync {
    /// Authenticate and establish a connection.
    async fn connect(&mut self) -> Result<()>;

    /// Tear down the connection.
    async fn disconnect(&mut self) -> Result<()>;

    /// Push local changes to the remote.
    async fn push(
        &self,
        notes: &[Note],
        embeddings: &HashMap<NoteId, Vec<f32>>,
        positions: &HashMap<NoteId, Position>,
        snapshot_id: Option<&str>,
    ) -> Result<SyncSnapshot>;

    /// Pull remote changes since the given snapshot.
    async fn pull(&self, since_snapshot: Option<&str>) -> Result<SyncPullResult>;

    /// Query the remote for storage stats and latest snapshot.
    async fn status(&self) -> Result<SyncStatus>;

    /// Timestamp of the last successful sync operation.
    fn last_sync(&self) -> Option<DateTime<Utc>> {
        None
    }
}
