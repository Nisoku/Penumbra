use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A point-in-time summary of the sync state.
///
/// The client stores the latest snapshot id locally to know what
/// changes it has already applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSnapshot {
    pub snapshot_id: String,
    pub timestamp: DateTime<Utc>,
    pub note_count: u64,
    pub note_ids: Vec<String>,
}

impl SyncSnapshot {
    pub fn new(note_ids: Vec<String>) -> Self {
        Self {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            note_count: note_ids.len() as u64,
            note_ids,
        }
    }
}
