use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use penumbra_core::error::Result;
use penumbra_core::link::Link;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_events::{Event, EventBus};
use penumbra_graph::GraphStore;
use penumbra_layout::LayoutEngine;
use penumbra_storage::Storage;
use penumbra_thread::{spawn_worker, Worker};

use crate::state::AppState;

const LAYOUT_INTERVAL_MS: u64 = 16;

/// Load the saved graph (notes + links) from storage.
pub async fn load_graph(storage: &Storage) -> Result<Option<(Vec<Note>, Vec<Link>)>> {
    storage.load_graph().await
}

/// Load saved positions from storage.
pub async fn load_positions(storage: &Storage) -> Result<Option<HashMap<NoteId, Position>>> {
    storage.load_positions().await
}

/// Restore app state from persisted data: insert notes, links, then emit
/// a `StateRestored` event with the saved positions so the canvas can
/// snap to them immediately.
pub async fn restore_state(
    state: &AppState,
    notes: Vec<Note>,
    links: Vec<Link>,
    positions: HashMap<NoteId, Position>,
) -> Result<()> {
    {
        let mut g = state.graph.lock().expect("graph lock poisoned");
        for note in &notes {
            g.add_note(note.clone());
        }
        for link in &links {
            let _ = g.link_notes(&link.source, &link.target, link.kind);
        }
    }

    for (id, note) in notes.iter().map(|n| (n.id, n)) {
        let embedding = state.embedder.embed_note(note).await?;
        let mut idx = state.index.lock().expect("index lock poisoned");
        let _ = idx.insert(id, &embedding);
    }

    state
        .event_bus
        .publish(Event::StateRestored { positions })
        .await;

    Ok(())
}

/// Create a [`LayoutEngine`] and populate it with all current graph nodes.
pub async fn create_layout_engine(state: &AppState) -> LayoutEngine {
    let mut engine = LayoutEngine::with_defaults().await;

    let (notes, links) = {
        let g = state.graph.lock().expect("graph lock poisoned");
        let notes: Vec<Note> = g.all_notes().cloned().collect();
        let links: Vec<Link> = g.all_links().into_iter().cloned().collect();
        (notes, links)
    };

    for note in &notes {
        engine.add_node(note.id, note.meta.pinned);
    }
    engine.update_links(links);

    engine
}

/// Spawn a background worker that steps the layout engine periodically.
///
/// Reads the current links from the shared graph on each cycle, syncs them
/// into the engine, calls [`LayoutEngine::step`], and publishes the
/// resulting positions through `event_bus` as [`Event::LayoutChanged`].
///
/// Sleeps longer when the layout has stabilised (displacement < 0.01) to
/// avoid unnecessary CPU work.
pub fn start_layout_worker(
    mut engine: LayoutEngine,
    graph: Arc<std::sync::Mutex<GraphStore>>,
    event_bus: Arc<EventBus>,
) -> Worker {
    spawn_worker("layout-worker", move |w| {
        let mut links_cache: Vec<Link> = Vec::new();
        let mut node_ids: Vec<NoteId> = Vec::new();

        while !w.is_cancelled() {
            let (current_links, current_notes): (Vec<Link>, Vec<Note>) = match graph.lock() {
                Ok(g) => (
                    g.all_links().into_iter().cloned().collect(),
                    g.all_notes().cloned().collect(),
                ),
                Err(_) => break,
            };

            // Sync links
            if current_links != links_cache {
                engine.update_links(current_links.clone());
                links_cache = current_links;
            }

            // Add new nodes that appeared since last cycle
            for note in &current_notes {
                if !node_ids.contains(&note.id) {
                    engine.add_node(note.id, note.meta.pinned);
                    node_ids.push(note.id);
                }
            }

            // Remove nodes that have been deleted
            let current_ids: Vec<NoteId> = current_notes.iter().map(|n| n.id).collect();
            node_ids.retain(|id| current_ids.contains(id));

            let displacement = engine.step();

            let positions = engine.all_positions();
            event_bus.try_publish(Event::LayoutChanged { positions });

            let step_ms = if displacement < 0.01 && engine.iteration_count() > 10 {
                LAYOUT_INTERVAL_MS * 4
            } else {
                LAYOUT_INTERVAL_MS
            };

            std::thread::sleep(Duration::from_millis(step_ms));
        }
    })
}
