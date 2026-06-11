use serde::{Deserialize, Serialize};

use crate::note::NoteId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    Explicit,
    Implicit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub source: NoteId,
    pub target: NoteId,
    pub kind: LinkKind,
    pub weight: f64,
}

impl Link {
    pub fn new(source: NoteId, target: NoteId, kind: LinkKind) -> Self {
        Self {
            source,
            target,
            kind,
            weight: if kind == LinkKind::Explicit { 1.0 } else { 0.5 },
        }
    }

    pub fn with_weight(source: NoteId, target: NoteId, kind: LinkKind, weight: f64) -> Self {
        Self {
            source,
            target,
            kind,
            weight,
        }
    }

    pub fn other(&self, id: &NoteId) -> Option<&NoteId> {
        if &self.source == id {
            Some(&self.target)
        } else if &self.target == id {
            Some(&self.source)
        } else {
            None
        }
    }
}
