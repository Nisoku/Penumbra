use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use dioxus::prelude::*;
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use penumbra_embed::candle::CandleEmbedder;
use penumbra_events::{Event, EventBus};
use penumbra_storage::Storage;

use crate::bridge;
use crate::state::AppState;

/// Initialize the application: create storage, app state, load persisted
/// data, start the layout worker, and subscribe to events.
///
/// All signals are created at the component's top level and passed in.
pub fn use_init(
    mut ctx: Signal<Option<Arc<AppState>>>,
    mut positions: Signal<HashMap<NoteId, Position>>,
    mut graph_version: Signal<u64>,
    mut ready: Signal<bool>,
    dragged_set: Signal<std::collections::HashSet<NoteId>>,
) {
    let mut started = use_signal(|| false);

    use_effect(move || {
        if *started.read() {
            return;
        }
        *started.write() = true;

        spawn(async move {
            let storage = Storage::new().await.expect("storage init failed");
            let embedder: Arc<dyn penumbra_core::EmbeddingProvider> = {
                match CandleEmbedder::load().await {
                    Ok(c) => Arc::new(c),
                    Err(e) => {
                        tracing::warn!(
                            "CandleEmbedder::load failed: {e}, falling back to SimpleEmbedder"
                        );
                        Arc::new(penumbra_embed::SimpleEmbedder::new_384())
                    }
                }
            };
            let state = AppState::new(embedder, storage)
                .await
                .expect("app state init failed");
            let state = Arc::new(state);

            // Restore saved graph + positions.
            let saved_positions = bridge::load_positions(&state.storage)
                .await
                .ok()
                .flatten()
                .unwrap_or_default();

            if let Ok(Some((notes, links))) = bridge::load_graph(&state.storage).await {
                bridge::restore_state(&state, notes, links, saved_positions.clone())
                    .await
                    .expect("state restore failed");

                positions.set(saved_positions.clone());
                let mut v = graph_version.write();
                *v = 1;
            }

            // Start the layout engine in a background worker.
            let mut engine = bridge::create_layout_engine(&state).await;

            // Feed saved positions into the engine so it starts from
            // persisted locations instead of random initial positions.
            for (id, pos) in &saved_positions {
                engine.set_position(id, *pos);
            }
            bridge::start_layout_worker(
                engine,
                Arc::clone(&state.graph),
                Arc::clone(&state.event_bus),
            );

            // Subscribe to layout -> position signal + persist with debounce.
            let bus = Arc::clone(&state.event_bus);
            let pos = positions;
            let saver = Arc::clone(&state);
            spawn(async move { layout_event_loop(bus, pos, saver, dragged_set).await });

            // Subscribe to graph mutations -> version bump.
            let bus = Arc::clone(&state.event_bus);
            let ver = graph_version;
            spawn(async move { graph_event_loop(bus, ver).await });

            ctx.set(Some(state));
            ready.set(true);
        });
    });
}

async fn layout_event_loop(
    event_bus: Arc<EventBus>,
    mut positions: Signal<HashMap<NoteId, Position>>,
    state: Arc<AppState>,
    dragged_set: Signal<std::collections::HashSet<NoteId>>,
) {
    let rx = event_bus.subscribe();
    let mut last_save = Instant::now();
    let debounce = std::time::Duration::from_secs(2);
    loop {
        match rx.recv().await {
            Ok(Event::LayoutChanged { positions: p }) => {
                // While a note is being dragged the layout engine runs with
                // stale force data, producing jitter for every non-dragged
                // note.  Skip the positions update until the drag ends so
                // that other notes stay still.
                if !dragged_set.read().is_empty() {
                    continue;
                }
                let mut merged = p.clone();
                let dragged = dragged_set.read().clone();
                for id in &dragged {
                    if let Some(pos) = positions.read().get(id) {
                        merged.insert(*id, *pos);
                    }
                }
                positions.set(merged);
                if last_save.elapsed() >= debounce {
                    if let Err(e) = state.storage.save_positions(&p).await {
                        tracing::error!("save positions: {e}");
                    }
                    last_save = Instant::now();
                }
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

async fn graph_event_loop(event_bus: Arc<EventBus>, mut version: Signal<u64>) {
    let rx = event_bus.subscribe();
    loop {
        match rx.recv().await {
            Ok(
                Event::NoteAdded { .. }
                | Event::NoteUpdated { .. }
                | Event::NoteRemoved { .. }
                | Event::NotePinned { .. }
                | Event::NoteUnpinned { .. }
                | Event::LinkAdded { .. }
                | Event::LinkRemoved { .. },
            ) => {
                *version.write() += 1;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}
