use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(Uuid);

impl NoteId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_raw(raw: Uuid) -> Self {
        Self(raw)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(default)]
    pub pinned: bool,

    #[serde(default)]
    pub archived: bool,
}

impl Default for NoteMeta {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            pinned: false,
            archived: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub meta: NoteMeta,
}

impl Note {
    pub fn new(title: String, body: String) -> Self {
        Self {
            id: NoteId::new(),
            title,
            body,
            tags: Vec::new(),
            meta: NoteMeta::default(),
        }
    }

    pub fn touch(&mut self) {
        self.meta.updated_at = Utc::now();
    }
}
