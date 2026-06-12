use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus::html::geometry::WheelDelta;
use dioxus_i18n::prelude::*;

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use unic_langid::langid;

use crate::components::{Fab, FloatingSidebar, GraphCards, NoteEditor, TopBar};
use crate::hooks::{use_camera, use_canvas, use_init, use_render_state};
use crate::state::AppState;

#[derive(Clone, Copy, PartialEq)]
enum AppMode {
    Graph,
    Editor { note_id: Option<NoteId> },
}

#[allow(non_snake_case)]
pub fn App() -> Element {
    // i18n
    use_init_i18n(|| {
        I18nConfig::new(langid!("en-US"))
            .with_locale((langid!("en-US"), include_str!("../locales/en-US.ftl")))
    });

    // Signals
    let app_state: Signal<Option<Arc<AppState>>> = use_signal(|| None);
    let mut positions: Signal<HashMap<NoteId, Position>> = use_signal(HashMap::new);
    let graph_version: Signal<u64> = use_signal(|| 0);
    let ready: Signal<bool> = use_signal(|| false);
    let window_size: Signal<(f32, f32)> = use_signal(|| (1280.0, 800.0));
    let selected: Signal<Option<NoteId>> = use_signal(|| None);
    let hovered: Signal<Option<NoteId>> = use_signal(|| None);
    let mut app_mode: Signal<AppMode> = use_signal(|| AppMode::Graph);

    // Note-creation card->zoom->editor transition
    let mut pending_note: Signal<Option<NoteId>> = use_signal(|| None);
    let mut drift_triggered: Signal<bool> = use_signal(|| false);

    // Camera
    let mut camera = use_camera();

    // Pan/zoom interaction state
    let mut is_panning = use_signal(|| false);
    let mut pan_start = use_signal(|| (0.0f64, 0.0f64));

    // Note dragging
    let mut dragging_note: Signal<Option<NoteId>> = use_signal(|| None);
    let drag_offset: Signal<(f64, f64)> = use_signal(|| (0.0, 0.0));

    // Center camera on viewport after init
    use_effect(move || {
        if !ready() { return };
        let (w, h) = *window_size.read();
        *camera.x.write() = w as f64 / 2.0;
        *camera.y.write() = h as f64 / 2.0;
    });

    // Initialization
    use_init(app_state, positions, graph_version, ready);

    // Render state
    let render_state = use_render_state(
        app_state,
        positions,
        camera.x,
        camera.y,
        camera.zoom,
        graph_version,
        selected,
        hovered,
    );

    // Canvas lifecycle
    use_canvas("penumbra-graph", render_state, window_size, ready);

    // Note creation: wait for position -> trigger camera drift
    use_effect(move || {
        let Some(note_id) = *pending_note.read() else {
            return;
        };
        if drift_triggered() {
            return;
        };
        let Some(pos) = positions.read().get(&note_id).copied() else {
            return;
        };
        camera.drift_to(pos);
        camera.drift_zoom(2.5);
        drift_triggered.set(true);
    });

    // Note creation: drift done -> switch to editor
    use_effect(move || {
        if !drift_triggered() {
            return;
        };
        if camera.is_drifting() {
            return;
        };
        let note_id = pending_note();
        app_mode.set(AppMode::Editor { note_id });
        pending_note.set(None);
        drift_triggered.set(false);
    });

    // Render
    let mode = *app_mode.read();
    match mode {
        AppMode::Graph => rsx! {
            div {
                style: "width: 100vw; height: 100vh; background: #0a0f1e; position: relative; overflow: hidden;",

                // Dot-grid background (CSS, decorative)
                div {
                    style: "position: absolute; inset: 0; \
                        background-image: radial-gradient(circle, rgba(99, 148, 220, 0.18) 1px, transparent 1px); \
                        background-size: 28px 28px; \
                        pointer-events: none;",
                }

                // Graph canvas
                canvas {
                    id: "penumbra-graph",
                    style: "position: absolute; inset: 0; width: 100%; height: 100%;",
                    onmousedown: move |evt: Event<MouseData>| {
                        if dragging_note().is_some() { return };
                        let coords = evt.data.client_coordinates();
                        is_panning.set(true);
                        pan_start.set((coords.x, coords.y));
                    },
                    onmousemove: move |evt: Event<MouseData>| {
                        let coords = evt.data.client_coordinates();
                        // Note dragging takes priority
                        if let Some(note_id) = *dragging_note.read() {
                            let zoom = *camera.zoom.read();
                            let (ox, oy) = *drag_offset.read();
                            let wx = (coords.x - *camera.x.read()) / zoom - ox;
                            let wy = (coords.y - *camera.y.read()) / zoom - oy;
                            positions.write().insert(note_id, Position::new(wx, wy));
                            return;
                        }
                        if !is_panning() { return };
                        let (sx, sy) = pan_start();
                        let dx = coords.x - sx;
                        let dy = coords.y - sy;
                        camera.pan(dx, dy);
                        pan_start.set((coords.x, coords.y));
                    },
                    onmouseup: move |_| {
                        is_panning.set(false);
                        dragging_note.set(None);
                    },
                    onmouseleave: move |_| {
                        is_panning.set(false);
                        dragging_note.set(None);
                    },
                    onwheel: move |evt: Event<WheelData>| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        let delta = evt.data.delta();
                        let dz = match delta {
                            WheelDelta::Pixels(v) => v.z,
                            WheelDelta::Lines(v) => v.z * 10.0,
                            WheelDelta::Pages(v) => v.z * 100.0,
                        };
                        if dz == 0.0 { return };
                        let coords = evt.data.client_coordinates();
                        camera.zoom_at(dz * -0.001, coords.x, coords.y);
                    },
                }

                GraphCards {
                    app_state,
                    positions,
                    camera,
                    dragging_note,
                    drag_offset,
                }

                TopBar {}
                FloatingSidebar {}
                Fab {
                    onclick: move |_| {
                        if !ready() {
                            tracing::warn!("not ready yet");
                            return;
                        }
                        let app_state = app_state;
                        let mut pending_note = pending_note;
                        spawn(async move {
                            let state = (*app_state.read()).clone();
                            match state {
                                Some(s) => match s.add_note(String::new(), String::new()).await {
                                    Ok(note) => {
                                        tracing::info!("created note {}", note.id);
                                        pending_note.set(Some(note.id));
                                    }
                                    Err(e) => tracing::error!("add_note: {e}"),
                                },
                                None => tracing::error!("app_state is None even though ready"),
                            }
                        });
                    },
                }
            }
        },
        AppMode::Editor { note_id } => rsx! {
            NoteEditor {
                app_state,
                note_id,
                on_back: move |_| {
                    app_mode.set(AppMode::Graph);
                },
            }
        },
    }
}
