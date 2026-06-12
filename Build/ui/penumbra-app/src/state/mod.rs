use std::sync::{Arc, Mutex};

use penumbra_auto_link::AutoLinker;
use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_core::EmbeddingProvider;
use penumbra_events::{Event, EventBus};
use penumbra_graph::GraphStore;
use penumbra_index::{RuvectorIndex, VectorIndex};
use penumbra_search::{SearchEngine, SearchResult};
use penumbra_storage::Storage;

const EMBED_DIMS: usize = 384;

pub struct AppState {
    pub graph: Arc<Mutex<GraphStore>>,
    pub index: Arc<Mutex<dyn VectorIndex>>,
    pub embedder: Arc<dyn EmbeddingProvider>,
    pub event_bus: Arc<EventBus>,
    pub search: SearchEngine,
    pub auto_link: AutoLinker,
    pub storage: Storage,
}

impl AppState {
    pub async fn new(embedder: Arc<dyn EmbeddingProvider>, storage: Storage) -> Result<Self> {
        let graph = Arc::new(Mutex::new(GraphStore::new()));
        let index = Arc::new(Mutex::new(RuvectorIndex::new(EMBED_DIMS)?));
        let event_bus = Arc::new(EventBus::new());

        let search = SearchEngine::new(
            Arc::clone(&embedder),
            Arc::clone(&index) as Arc<Mutex<dyn VectorIndex>>,
        );
        let auto_link = AutoLinker::with_defaults(
            Arc::clone(&embedder),
            Arc::clone(&index) as Arc<Mutex<dyn VectorIndex>>,
            Arc::clone(&graph),
            Arc::clone(&event_bus),
        );

        Ok(Self {
            graph,
            index,
            embedder,
            event_bus,
            search,
            auto_link,
            storage,
        })
    }

    pub async fn add_note(&self, title: String, body: String) -> Result<Note> {
        let note = Note::new(title, body);
        let id = note.id;

        {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.add_note(note.clone());
        }

        let embedding = self.embedder.embed_note(&note).await?;
        {
            let mut idx = self
                .index
                .lock()
                .map_err(|e| PenumbraError::Index(format!("index lock poisoned: {e}")))?;
            idx.insert(id, &embedding)?;
        }

        self.storage.save_note(&note).await?;

        let _links = self.auto_link.process_note(&note).await?;

        self.event_bus
            .publish(Event::NoteAdded {
                id,
                note: note.clone(),
            })
            .await;

        self.save_graph().await?;

        Ok(note)
    }

    pub async fn remove_note(&self, id: &NoteId) -> Result<Option<Note>> {
        let note = {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.remove_note(id)
        };

        if note.is_some() {
            {
                let mut idx = self
                    .index
                    .lock()
                    .map_err(|e| PenumbraError::Index(format!("index lock poisoned: {e}")))?;
                let _ = idx.remove(id);
            }

            self.storage.delete_note(id).await?;
            self.save_graph().await?;

            self.event_bus.publish(Event::NoteRemoved { id: *id }).await;
        }

        Ok(note)
    }

    pub async fn update_note(
        &self,
        id: &NoteId,
        title: Option<String>,
        body: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<()> {
        {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            let note = g
                .get_note_mut(id)
                .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?;

            if let Some(t) = title {
                note.title = t;
            }
            if let Some(b) = body {
                note.body = b;
            }
            if let Some(t) = tags {
                note.tags = t;
            }
            note.touch();
        }

        let note = {
            let g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.get_note(id)
                .cloned()
                .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?
        };

        let embedding = self.embedder.embed_note(&note).await?;
        {
            let mut idx = self
                .index
                .lock()
                .map_err(|e| PenumbraError::Index(format!("index lock poisoned: {e}")))?;
            idx.insert(*id, &embedding)?;
        }

        self.storage.save_note(&note).await?;

        self.event_bus
            .publish(Event::NoteUpdated { id: *id, note })
            .await;

        self.save_graph().await?;

        Ok(())
    }

    pub async fn toggle_pin(&self, id: &NoteId) -> Result<bool> {
        let pinned = {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            let note = g
                .get_note_mut(id)
                .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?;
            note.meta.pinned = !note.meta.pinned;
            note.touch();
            note.meta.pinned
        };
        let note = {
            let g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.get_note(id)
                .cloned()
                .ok_or_else(|| PenumbraError::NoteNotFound(id.to_string()))?
        };
        self.storage.save_note(&note).await?;
        if pinned {
            self.event_bus.try_publish(Event::NotePinned { id: *id });
        } else {
            self.event_bus.try_publish(Event::NoteUnpinned { id: *id });
        }
        Ok(pinned)
    }

    pub fn link_notes(&self, source: &NoteId, target: &NoteId, kind: LinkKind) -> Result<Link> {
        let link = {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.link_notes(source, target, kind)?
        };

        self.event_bus
            .try_publish(Event::LinkAdded { link: link.clone() });

        Ok(link)
    }

    pub fn unlink_notes(&self, source: &NoteId, target: &NoteId) -> Result<Link> {
        let link = {
            let mut g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.unlink_notes(source, target)?
        };

        self.event_bus.try_publish(Event::LinkRemoved {
            source: *source,
            target: *target,
        });

        Ok(link)
    }

    pub async fn search(&self, query: &str, tags: &[String]) -> Result<Vec<SearchResult>> {
        let notes: Vec<Note> = {
            let g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            g.all_notes().cloned().collect()
        };

        self.search.search(query, &notes, tags).await
    }

    pub fn get_position(&self, _id: &NoteId) -> Option<Position> {
        // Positions are managed externally by the layout engine and
        // delivered via Event::LayoutChanged. This is a placeholder
        // for a position cache if we add one later.
        None
    }

    async fn save_graph(&self) -> Result<()> {
        let (notes, links) = {
            let g = self
                .graph
                .lock()
                .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
            let notes: Vec<Note> = g.all_notes().cloned().collect();
            let links: Vec<Link> = g.all_links().into_iter().cloned().collect();
            (notes, links)
        };
        self.storage.save_graph(&notes, &links).await
    }

    pub fn all_notes(&self) -> Result<Vec<Note>> {
        let g = self
            .graph
            .lock()
            .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
        Ok(g.all_notes().cloned().collect())
    }

    pub fn all_links(&self) -> Result<Vec<Link>> {
        let g = self
            .graph
            .lock()
            .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
        Ok(g.all_links().into_iter().cloned().collect())
    }
}
