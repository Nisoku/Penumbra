use std::sync::{Arc, Mutex};

use penumbra_core::{
    link::{Link, LinkKind},
    note::Note,
    EmbeddingProvider, PenumbraError, Result,
};
use penumbra_events::{Event, EventBus};
use penumbra_graph::GraphStore;
use penumbra_index::VectorIndex;

/// Configuration for the auto-linking behaviour.
#[derive(Debug, Clone)]
pub struct AutoLinkConfig {
    /// How many candidate neighbours to retrieve from the vector index.
    pub top_k: usize,
    /// Minimum cosine-similarity score for an implicit link to be created.
    pub min_score: f32,
    /// Maximum number of implicit links to create per note in a single pass.
    pub max_links: usize,
}

impl Default for AutoLinkConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_score: 0.75,
            max_links: 5,
        }
    }
}

/// Orchestrates the auto-linking pipeline: embed -> search -> link.
///
/// On each `process_note` call the note's text is embedded, the vector index
/// is searched for the closest neighbours, and implicit links are created in
/// the graph for every neighbour whose similarity exceeds the configured
/// threshold.  Existing explicit links are never removed.
pub struct AutoLinker {
    embedder: Arc<dyn EmbeddingProvider>,
    index: Arc<Mutex<dyn VectorIndex>>,
    graph: Arc<Mutex<GraphStore>>,
    event_bus: Arc<EventBus>,
    config: AutoLinkConfig,
}

impl AutoLinker {
    pub fn new(
        embedder: Arc<dyn EmbeddingProvider>,
        index: Arc<Mutex<dyn VectorIndex>>,
        graph: Arc<Mutex<GraphStore>>,
        event_bus: Arc<EventBus>,
        config: AutoLinkConfig,
    ) -> Self {
        Self {
            embedder,
            index,
            graph,
            event_bus,
            config,
        }
    }

    /// Convenience constructor with default config.
    pub fn with_defaults(
        embedder: Arc<dyn EmbeddingProvider>,
        index: Arc<Mutex<dyn VectorIndex>>,
        graph: Arc<Mutex<GraphStore>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self::new(embedder, index, graph, event_bus, AutoLinkConfig::default())
    }

    /// Embed `note`, search the index, and create implicit links.
    ///
    /// The note's embedding is inserted into the index *before* searching so
    /// that it is immediately discoverable by subsequent saves.  The note is
    /// filtered out of its own results.
    pub async fn process_note(&self, note: &Note) -> Result<Vec<Link>> {
        let embedding = self.embedder.embed_note(note).await?;

        // Index the new embedding so it is visible from now on.
        {
            let mut idx = self
                .index
                .lock()
                .map_err(|e| PenumbraError::Index(format!("index lock poisoned: {e}")))?;
            idx.insert(note.id, &embedding)?;
        }

        // Search for the closest neighbours.
        let hits = {
            let idx = self
                .index
                .lock()
                .map_err(|e| PenumbraError::Index(format!("index lock poisoned: {e}")))?;
            idx.search(&embedding, self.config.top_k)?
        };

        let mut created = Vec::new();

        for hit in hits {
            // Skip self.
            if hit.id == note.id {
                continue;
            }
            // Score floor.
            if hit.score < self.config.min_score {
                continue;
            }
            if created.len() >= self.config.max_links {
                break;
            }

            // Double-check that the link does not already exist (another
            // thread may have created it between our search and now).
            {
                let graph = self
                    .graph
                    .lock()
                    .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;

                let already_linked = graph
                    .get_links(&note.id)
                    .iter()
                    .any(|l| l.source == hit.id || l.target == hit.id);

                if already_linked {
                    continue;
                }
            }

            // Create the implicit link.
            let link = {
                let mut graph = self
                    .graph
                    .lock()
                    .map_err(|e| PenumbraError::Graph(format!("graph lock poisoned: {e}")))?;
                graph.link_notes(&note.id, &hit.id, LinkKind::Implicit)?
            };

            self.event_bus
                .publish(Event::LinkAdded { link: link.clone() })
                .await;

            tracing::info!(
                "auto-link: {} <-> {} (score={})",
                note.id,
                hit.id,
                hit.score
            );

            created.push(link);
        }

        Ok(created)
    }
}
