use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub color: Option<String>,
}

impl Tag {
    pub fn new(name: String) -> Self {
        Self { name, color: None }
    }

    pub fn with_color(name: String, color: String) -> Self {
        Self {
            name,
            color: Some(color),
        }
    }
}

impl From<String> for Tag {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}
