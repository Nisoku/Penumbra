use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub content: String,
    pub tags: Vec<String>,
    pub links: Vec<Uuid>,
    pub position: Option<(f64, f64)>,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Note {
    pub fn new(content: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4(),
            content,
            tags: Vec::new(),
            links: Vec::new(),
            position: None,
            pinned: false,
            created_at: now,
            updated_at: now,
        }
    }
}