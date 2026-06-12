use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use penumbra_canvas::{Camera, RenderEdge, RenderNode, RenderState};
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

use crate::state::AppState;

/// Merge raw data into a single `RenderState` signal.
///
/// Rebuilds whenever camera, positions, graph version, or selection changes.
pub fn use_render_state(
    app_state: Signal<Option<Arc<AppState>>>,
    positions: Signal<HashMap<NoteId, Position>>,
    camera_x: Signal<f64>,
    camera_y: Signal<f64>,
    camera_zoom: Signal<f64>,
    graph_version: Signal<u64>,
    selected: Signal<Option<NoteId>>,
    hovered: Signal<Option<NoteId>>,
) -> Signal<RenderState> {
    let mut state: Signal<RenderState> = use_signal(RenderState::default);

    use_effect(move || {
        // Track signal dependencies
        let px = *camera_x.read();
        let py = *camera_y.read();
        let pz = *camera_zoom.read();
        let _ = positions();
        let _ = graph_version();
        let sel = selected();
        let hov = hovered();

        let Some(ref app) = *app_state.read() else {
            return;
        };

        let notes = app.all_notes().unwrap_or_default();
        let links = app.all_links().unwrap_or_default();
        let pos = positions();

        let cam = Camera {
            x: px,
            y: py,
            zoom: pz,
        };

        let nodes: Vec<RenderNode> = notes
            .iter()
            .map(|n| RenderNode {
                id: n.id,
                position: pos.get(&n.id).copied().unwrap_or_default(),
                title: n.title.clone(),
                tags: n.tags.clone(),
                pinned: n.meta.pinned,
            })
            .collect();

        let edges: Vec<RenderEdge> = links
            .iter()
            .map(|l| RenderEdge {
                source: l.source,
                target: l.target,
                opacity: if l.kind == penumbra_core::link::LinkKind::Implicit {
                    0.35
                } else {
                    0.7
                },
            })
            .collect();

        state.set(RenderState {
            camera: cam,
            nodes,
            edges,
            hovered_node: hov,
            selected_node: sel,
        });
    });

    state
}
