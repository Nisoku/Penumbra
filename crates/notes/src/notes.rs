use crate::Note;
use std::collections::HashMap;

pub struct Notes {
    notes: HashMap<uuid::Uuid, Note>,
}

impl Notes {
    pub fn new() -> Self {
        Self {
            notes: HashMap::new(),
        }
    }

    pub fn add(&mut self, note: Note) -> uuid::Uuid {
        let id = note.id;
        self.notes.insert(id, note);
        id
    }

    pub fn get(&self, id: uuid::Uuid) -> Option<&Note> {
        self.notes.get(&id)
    }

    pub fn get_mut(&mut self, id: uuid::Uuid) -> Option<&mut Note> {
        self.notes.get_mut(&id)
    }

    pub fn all(&self) -> Vec<&Note> {
        self.notes.values().collect()
    }

    pub fn count(&self) -> usize {
        self.notes.len()
    }
}