use std::sync::Arc;

use dioxus::prelude::*;
use penumbra_canvas::RenderState;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_embed::SimpleEmbedder;
use penumbra_events::{Event, EventBus};
use penumbra_layout::LayoutEngine;
use penumbra_storage::Storage;
use penumbra_thread::Worker;

use crate::bridge;
use crate::state::AppState;

pub struct AppHandle {
    pub state: AppState,
    pub render_state: Signal<RenderState>,
    pub positions: Signal<std::collections::HashMap<NoteId, Position>>,
    pub _layout_worker: Worker,
}

/// Initialize the full application: storage, graph, index, embedder,
/// layout engine, and background workers.
///
/// Returns `None` while loading, `Some(AppHandle)` once ready.
pub fn use_initialize_app() -> Signal<Option<AppHandle>> {
    let handle: Signal<Option<AppHandle>> = use_signal(|| None);

    let resource = use_resource(move || async move {
        if handle().is_some() {
            return;
        }

        let storage = Storage::new()
            .await
            .expect("failed to initialize storage");
        let embedder = Arc::new(SimpleEmbedder::new_384());
        let state = AppState::new(embedder, storage)
            .await
            .expect("failed to initialize app state");

        let (saved_notes, saved_links, saved_positions) = load_persisted(&state).await;

        bridge::restore_state(
            &state,
            saved_notes,
            saved_links,
            saved_positions.clone(),
        )
        .await
        .expect("failed to restore state");

        let engine = bridge::create_layout_engine(&state).await;
        let layout_worker = bridge::start_layout_worker(
            engine,
            Arc::clone(&state.graph),
            Arc::clone(&state.event_bus),
        );

        let positions_signal: Signal<std::collections::HashMap<NoteId, Position>> =
            use_signal(|| saved_positions);

        let event_bus = Arc::clone(&state.event_bus);
        spawn(async move {
            subscribe_layout_events(event_bus, positions_signal).await;
        });

        let render_state: Signal<RenderState> = use_signal(RenderState::default);

        handle.set(Some(AppHandle {
            state,
            render_state,
            positions: positions_signal,
            _layout_worker: layout_worker,
        }));
    });

    handle
}

async fn load_persisted(
    state: &AppState,
) -> (Vec<Note>, Vec<penumbra_core::link::Link>, std::collections::HashMap<NoteId, Position>) {
    let graph = bridge::load_graph(&state.storage)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    let positions = bridge::load_positions(&state.storage)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    (graph.0, graph.1, positions)
}

async fn subscribe_layout_events(
    event_bus: Arc<EventBus>,
    positions: Signal<std::collections::HashMap<NoteId, Position>>,
) {
    let rx = event_bus.subscribe();
    loop {
        match rx.recv().await {
            Ok(Event::LayoutChanged { positions: new_pos }) => {
                positions.set(new_pos);
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}
