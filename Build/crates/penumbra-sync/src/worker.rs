use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::{PenumbraError, Result};
use uuid::Uuid;

use crate::provider::{SyncProvider, SyncPullResult, SyncStatus};
use crate::snapshot::SyncSnapshot;

/// HTTP client that synchronises with a Cloudflare Worker backend.
///
/// The worker stores notes, embeddings and positions in R2 behind a
/// simple REST API.  This provider speaks that same API.
pub struct WorkerSyncProvider {
    base_url: String,
    client: reqwest::Client,
    last_sync: Mutex<Option<DateTime<Utc>>>,
}

impl WorkerSyncProvider {
    /// Point at the deployed Worker URL
    /// (`https://penumbra-sync.neeljaiswal23.workers.dev`).
    /// or maybe (`https://penumbra-sync.nisoku.org`)
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            last_sync: Mutex::new(None),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

#[async_trait(?Send)]
impl SyncProvider for WorkerSyncProvider {
    async fn connect(&mut self) -> Result<()> {
        let resp = self
            .client
            .get(self.url("/sync/status"))
            .send()
            .await
            .map_err(|e| PenumbraError::Sync(format!("cannot reach worker: {e}")))?;

        if !resp.status().is_success() {
            return Err(PenumbraError::Sync(format!(
                "worker returned {}",
                resp.status()
            )));
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn push(
        &self,
        notes: &[Note],
        embeddings: &HashMap<NoteId, Vec<f32>>,
        positions: &HashMap<NoteId, Position>,
        snapshot_id: Option<&str>,
    ) -> Result<SyncSnapshot> {
        let mut notes_map = serde_json::Map::new();
        for note in notes {
            let json = serde_json::to_value(note)
                .map_err(|e| PenumbraError::Sync(format!("serialize note: {e}")))?;
            notes_map.insert(note.id.to_string(), json);
        }

        let mut emb_map = serde_json::Map::new();
        for (id, vec) in embeddings {
            emb_map.insert(id.to_string(), serde_json::Value::from(vec.as_slice()));
        }

        let mut pos_map = serde_json::Map::new();
        for (id, pos) in positions {
            let json = serde_json::to_value(pos)
                .map_err(|e| PenumbraError::Sync(format!("serialize position: {e}")))?;
            pos_map.insert(id.to_string(), json);
        }

        let mut body = serde_json::json!({
            "notes": notes_map,
            "embeddings": emb_map,
            "positions": pos_map,
        });

        if let Some(sid) = snapshot_id {
            body["snapshotId"] = serde_json::Value::String(sid.to_string());
        }

        let resp = self
            .client
            .post(self.url("/sync/push"))
            .json(&body)
            .send()
            .await
            .map_err(|e| PenumbraError::Sync(format!("push request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(PenumbraError::Sync(format!(
                "push rejected: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PenumbraError::Sync(format!("parse push response: {e}")))?;

        let new_snapshot_id = data["snapshotId"]
            .as_str()
            .ok_or_else(|| PenumbraError::Sync("missing snapshotId in push response".into()))?
            .to_string();

        let accepted = data["accepted"].as_u64().unwrap_or(0);

        let snapshot = SyncSnapshot {
            snapshot_id: new_snapshot_id,
            timestamp: Utc::now(),
            note_count: accepted,
            note_ids: notes.iter().map(|n| n.id.to_string()).collect(),
        };

        *self.last_sync.lock().unwrap() = Some(Utc::now());

        Ok(snapshot)
    }

    async fn pull(&self, since_snapshot: Option<&str>) -> Result<SyncPullResult> {
        let mut url = self.url("/sync/pull");
        if let Some(sid) = since_snapshot {
            url.push_str(&format!("?since={}", urlencoding(sid)));
        }

        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| PenumbraError::Sync(format!("pull request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(PenumbraError::Sync(format!(
                "pull rejected: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PenumbraError::Sync(format!("parse pull response: {e}")))?;

        let mut notes = HashMap::new();
        if let Some(notes_map) = data["notes"].as_object() {
            for (_id_str, val) in notes_map {
                if let Ok(note) = serde_json::from_value::<Note>(val.clone()) {
                    notes.insert(note.id, note);
                }
            }
        }

        let mut embeddings = HashMap::new();
        if let Some(emb_map) = data["embeddings"].as_object() {
            for (id_str, val) in emb_map {
                if let Some(arr) = val.as_array() {
                    let vec: Vec<f32> = arr
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();
                    if let Ok(id) = Uuid::parse_str(id_str).map(NoteId::from_raw) {
                        embeddings.insert(id, vec);
                    }
                }
            }
        }

        let mut positions = HashMap::new();
        if let Some(pos_map) = data["positions"].as_object() {
            for (id_str, val) in pos_map {
                if let Ok(pos) = serde_json::from_value::<Position>(val.clone()) {
                    if let Ok(id) = Uuid::parse_str(id_str).map(NoteId::from_raw) {
                        positions.insert(id, pos);
                    }
                }
            }
        }

        let snapshot = if let Some(snap_val) = data.get("snapshot") {
            serde_json::from_value::<SyncSnapshot>(snap_val.clone()).ok()
        } else {
            None
        };

        *self.last_sync.lock().unwrap() = Some(Utc::now());

        Ok(SyncPullResult {
            notes,
            embeddings,
            positions,
            snapshot,
        })
    }

    async fn status(&self) -> Result<SyncStatus> {
        let resp = self
            .client
            .get(self.url("/sync/status"))
            .send()
            .await
            .map_err(|e| PenumbraError::Sync(format!("status request failed: {e}")))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PenumbraError::Sync(format!("parse status response: {e}")))?;

        let last_modified = data["lastModified"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(SyncStatus {
            note_count: data["noteCount"].as_u64().unwrap_or(0),
            last_modified,
            storage_bytes: data["storageBytes"].as_u64().unwrap_or(0),
            storage_limit: data["storageLimit"].as_u64().unwrap_or(512 * 1024 * 1024),
            snapshot_id: data["snapshotId"].as_str().map(|s| s.to_string()),
        })
    }

    fn last_sync(&self) -> Option<DateTime<Utc>> {
        *self.last_sync.lock().unwrap()
    }
}

/// Minimal percent-encoding for snapshot ids in query strings.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
