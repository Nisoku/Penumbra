use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use penumbra_embed::SimpleEmbedder;
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
) {
    let mut started = use_signal(|| false);

    use_effect(move || {
        if *started.read() {
            return;
        }
        *started.write() = true;

        spawn(async move {
            let storage = Storage::new().await.expect("storage init failed");
            let embedder: Arc<dyn penumbra_core::EmbeddingProvider> =
                Arc::new(SimpleEmbedder::new_384());
            let state = AppState::new(embedder, storage)
                .await
                .expect("app state init failed");
            let state = Arc::new(state);

            // Restore saved graph + positions.
            if let Ok(Some((notes, links))) = bridge::load_graph(&state.storage).await {
                let saved_positions = bridge::load_positions(&state.storage)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                bridge::restore_state(&state, notes, links, saved_positions.clone())
                    .await
                    .expect("state restore failed");

                positions.set(saved_positions);
                let mut v = graph_version.write();
                *v = 1;
            }

            // Start the layout engine in a background worker.
            let engine = bridge::create_layout_engine(&state).await;
            bridge::start_layout_worker(
                engine,
                Arc::clone(&state.graph),
                Arc::clone(&state.event_bus),
            );

            // Subscribe to layout -> position signal.
            let bus = Arc::clone(&state.event_bus);
            let pos = positions;
            spawn(async move { layout_event_loop(bus, pos).await });

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
) {
    let rx = event_bus.subscribe();
    loop {
        match rx.recv().await {
            Ok(Event::LayoutChanged { positions: p }) => positions.set(p),
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
