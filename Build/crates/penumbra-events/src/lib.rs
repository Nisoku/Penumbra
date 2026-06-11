use std::sync::Mutex;

use async_channel::{Receiver, Sender};
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use penumbra_core::{Link, Note};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Event {
    NoteAdded {
        id: NoteId,
        note: Note,
    },
    NoteUpdated {
        id: NoteId,
        note: Note,
    },
    NoteRemoved {
        id: NoteId,
    },
    NotePinned {
        id: NoteId,
    },
    NoteUnpinned {
        id: NoteId,
    },
    LinkAdded {
        link: Link,
    },
    LinkRemoved {
        source: NoteId,
        target: NoteId,
    },
    LayoutChanged {
        positions: HashMap<NoteId, Position>,
    },
    EmbeddingReady {
        id: NoteId,
    },
    /// Emitted during startup or after sync to restore the full canvas state.
    StateRestored {
        positions: HashMap<NoteId, Position>,
    },
}

pub struct EventBus {
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = async_channel::unbounded();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    pub async fn publish(&self, event: Event) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.try_send(event.clone()).is_ok());
    }

    pub fn try_publish(&self, event: Event) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.try_send(event.clone()).is_ok());
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
