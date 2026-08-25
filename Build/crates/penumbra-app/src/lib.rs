//! Application orchestrator.

use std::sync::Arc;

use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::note::{Note, NoteId};
use penumbra_events::{Event, EventBus};
use penumbra_graph::GraphStore;
use penumbra_storage::Storage;

/// Owns the graph, storage, and event bus for one running Penumbra instance.
pub struct Universe {
    graph: GraphStore,
    storage: Storage,
    events: Arc<EventBus>,
}

impl Universe {
    /// Open a universe backed by the platform app-data directory.
    pub async fn open_default() -> Result<Self> {
        let storage = Storage::new().await?;
        Self::open(storage).await
    }

    /// Restore a universe from storage, or start empty when nothing is saved.
    pub async fn open(storage: Storage) -> Result<Self> {
        let mut universe = Self {
            graph: GraphStore::new(),
            storage,
            events: Arc::new(EventBus::new()),
        };
        universe.restore_graph().await?;
        Ok(universe)
    }

    /// Subscribe to domain events emitted by mutations on this universe.
    pub fn events(&self) -> Arc<EventBus> {
        Arc::clone(&self.events)
    }

    /// Read-only access to the restored graph.
    pub fn graph(&self) -> &GraphStore {
        &self.graph
    }

    /// Number of notes currently held in memory.
    pub fn note_count(&self) -> usize {
        self.graph.note_count()
    }

    /// Create a note, persist it, register it in the graph, publish the event.
    pub async fn create_note(&mut self, title: String, body: String) -> Result<NoteId> {
        let note = Note::new(title, body);
        self.storage.save_note(&note).await?;
        if !self.graph.add_note(note.clone()) {
            return Err(PenumbraError::Graph(format!(
                "note {} already exists",
                note.id
            )));
        }
        self.persist_snapshot().await?;
        let id = note.id;
        self.events.publish(Event::NoteAdded { id, note }).await;
        Ok(id)
    }

    /// Persist a full note update coming from an editor session.
    pub async fn save_note(&mut self, note: Note) -> Result<()> {
        if self.graph.get_note(&note.id).is_some() {
            self.graph.update_note(&note.id, |existing| {
                *existing = note.clone();
            })?;
        } else if !self.graph.add_note(note.clone()) {
            return Err(PenumbraError::Graph(format!(
                "note {} already exists",
                note.id
            )));
        }
        self.storage.save_note(&note).await?;
        self.persist_snapshot().await?;
        let id = note.id;
        self.events.publish(Event::NoteUpdated { id, note }).await;
        Ok(())
    }

    /// Remove a note from disk, graph, and publish the removal.
    pub async fn delete_note(&mut self, id: &NoteId) -> Result<()> {
        let removed = self
            .graph
            .remove_note(id)
            .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?;
        self.storage.delete_note(id).await?;
        self.persist_snapshot().await?;
        self.events
            .publish(Event::NoteRemoved { id: removed.id })
            .await;
        Ok(())
    }

    async fn restore_graph(&mut self) -> Result<()> {
        match self.storage.load_graph().await? {
            Some((notes, links)) => {
                tracing::info!(
                    "restoring universe: {} notes, {} links",
                    notes.len(),
                    links.len()
                );
                self.graph
                    .restore(penumbra_graph::GraphSnapshot { notes, links });
            }
            None => tracing::info!("no saved universe found, starting empty"),
        }
        Ok(())
    }

    async fn persist_snapshot(&self) -> Result<()> {
        let snapshot = self.graph.snapshot();
        self.storage
            .save_graph(&snapshot.notes, &snapshot.links)
            .await
    }
}
