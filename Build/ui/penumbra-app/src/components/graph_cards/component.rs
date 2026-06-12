use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_motion::prelude::*;

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

use crate::components::note_card::NoteCard;
use crate::hooks::CameraHandle;
use crate::state::AppState;

#[allow(non_snake_case)]
#[component]
pub fn GraphCards(
    app_state: Signal<Option<Arc<AppState>>>,
    positions: Signal<HashMap<NoteId, Position>>,
    camera: CameraHandle,
    dragging_note: Signal<Option<NoteId>>,
    drag_offset: Signal<(f64, f64)>,
) -> Element {
    let cx = *camera.x.read();
    let cy = *camera.y.read();
    let zoom = *camera.zoom.read();

    let notes = match &*app_state.read() {
        Some(state) => {
            let g = state.graph.lock().expect("graph lock poisoned");
            g.all_notes().cloned().collect::<Vec<_>>()
        }
        None => Vec::new(),
    };

    let container_style = format!(
        "transform: translate({cx}px, {cy}px) scale({zoom}); \
         transform-origin: 0 0; \
         position: absolute; inset: 0; \
         pointer-events: none; overflow: visible;"
    );

    rsx! {
        div {
            style: "{container_style}",
            for note in &notes {
                if let Some(pos) = positions.read().get(&note.id).copied() {
                    AnimatedCard {
                        key: "{note.id}",
                        title: note.title.clone(),
                        preview: note.body.clone(),
                        x: pos.x,
                        y: pos.y,
                        id: note.id,
                        camera,
                        dragging_note,
                        drag_offset,
                    }
                }
            }
        }
    }
}

#[allow(non_snake_case)]
#[component]
fn AnimatedCard(
    title: String,
    preview: String,
    x: f64,
    y: f64,
    id: NoteId,
    camera: CameraHandle,
    dragging_note: Signal<Option<NoteId>>,
    drag_offset: Signal<(f64, f64)>,
) -> Element {
    let mut spring = use_motion(0.0f32);

    use_effect(move || {
        spring.animate_to(
            1.0,
            AnimationConfig::new(AnimationMode::Spring(Spring {
                stiffness: 180.0,
                damping: 18.0,
                ..Default::default()
            })),
        );
    });

    let s = spring.get_value() as f64;

    rsx! {
        div {
            style: "position: absolute; transform: translate({x}px, {y}px); \
                     pointer-events: auto;",
            onmousedown: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                let coords = evt.data.client_coordinates();
                let zoom = *camera.zoom.read();
                // Offset from note center to mouse in world space
                let mx = (coords.x - *camera.x.read()) / zoom;
                let my = (coords.y - *camera.y.read()) / zoom;
                drag_offset.set((mx - x, my - y));
                dragging_note.set(Some(id));
            },
            div {
                style: "transform: scale({s}); opacity: {s}; transform-origin: center center; \
                         pointer-events: none;",
                NoteCard {
                    title,
                    preview,
                    x: 0.0,
                    y: 0.0,
                }
            }
        }
    }
}
