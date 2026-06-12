use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus::html::geometry::WheelDelta;
use dioxus_i18n::prelude::*;
use dioxus_icons::lucide::{Link2, Maximize, Minus, Plus};

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use penumbra_events as pevents;
use penumbra_theme::Theme;
use unic_langid::langid;

use crate::hooks::CameraHandle;

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

    // Theme mode provided as context so nested components can read/write it.
    let theme_mode: Signal<String> = use_context_provider(|| Signal::new("dark".to_string()));

    // Apply theme CSS vars whenever mode changes.
    use_effect(move || {
        let mode = theme_mode();
        let theme = if mode == "light" {
            Theme::light()
        } else {
            Theme::dark()
        };
        let css = theme.css_root_block();
        _ = document::eval(&format!(
            "var s=document.getElementById('__pnb_theme');\
             if(!s){{s=document.createElement('style');s.id='__pnb_theme';\
             document.head.appendChild(s);}}s.textContent='{css}';"
        ));
    });

    // Signals
    let app_state: Signal<Option<Arc<AppState>>> = use_signal(|| None);
    let mut positions: Signal<HashMap<NoteId, Position>> = use_signal(HashMap::new);
    let graph_version: Signal<u64> = use_signal(|| 0);
    let ready: Signal<bool> = use_signal(|| false);
    let window_size: Signal<(f32, f32)> = use_signal(|| (1280.0, 800.0));
    let mut selected: Signal<Option<NoteId>> = use_signal(|| None);
    let hovered: Signal<Option<NoteId>> = use_signal(|| None);
    let mut app_mode: Signal<AppMode> = use_signal(|| AppMode::Graph);

    // Note-creation: canvas card -> drift -> editor
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
    let mut dragged_set: Signal<HashSet<NoteId>> = use_signal(HashSet::new);

    // Tag filter for graph view
    let selected_tag: Signal<Option<String>> = use_signal(|| None);

    // Manual-linking mode: Some(id) means "next clicked note links to id".
    let mut linking_from: Signal<Option<NoteId>> = use_signal(|| None);

    // How many notes currently exist (drives the empty-state overlay).
    let note_count = use_memo(move || {
        let _ = graph_version();
        app_state
            .read()
            .as_ref()
            .map(|s| s.all_notes().map(|n| n.len()).unwrap_or(0))
            .unwrap_or(0)
    });

    // Center camera on viewport after init
    use_effect(move || {
        if !ready() {
            return;
        }
        let (w, h) = *window_size.read();
        *camera.x.write() = w as f64 / 2.0;
        *camera.y.write() = h as f64 / 2.0;
    });

    // Initialization
    use_init(app_state, positions, graph_version, ready, dragged_set);

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


    // Reusable "create a new note" action (FAB + Cmd/Ctrl+N).
    let create_note = move || {
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
    };

    let mode = *app_mode.read();
    match mode {
        AppMode::Graph => rsx! {
            div {
                tabindex: "0",
                class: "pnb-themed",
                style: "width: 100vw; height: 100vh; background: var(--bg); position: relative; overflow: hidden; outline: none;",
                onmounted: move |evt: Event<MountedData>| {
                    // Focus the root so keyboard shortcuts are captured.
                    spawn(async move { let _ = evt.data().set_focus(true).await; });
                },
                onkeydown: move |evt: Event<KeyboardData>| {
                    let mods = evt.modifiers();
                    match evt.key() {
                        Key::Escape => {
                            if linking_from().is_some() {
                                linking_from.set(None);
                            }
                        }
                        Key::Character(c) if c == "n" && (mods.meta() || mods.ctrl()) => {
                            evt.prevent_default();
                            create_note();
                        }
                        _ => {}
                    }
                },
                onmousemove: move |evt: Event<MouseData>| {
                    let coords = evt.data.client_coordinates();
                    if let Some(note_id) = *dragging_note.read() {
                        let zoom = *camera.zoom.read();
                        let (ox, oy) = *drag_offset.read();
                        let wx = (coords.x - *camera.x.read()) / zoom - ox;
                        let wy = (coords.y - *camera.y.read()) / zoom - oy;
                        let pos = Position::new(wx, wy);
                        positions.write().insert(note_id, pos);
                        if let Some(ref s) = *app_state.read() {
                            s.event_bus.try_publish(
                                pevents::Event::SetNodePosition { id: note_id, position: pos },
                            );
                        }
                        return;
                    }
                    if !is_panning() {
                        return;
                    }
                    let (sx, sy) = pan_start();
                    let dx = coords.x - sx;
                    let dy = coords.y - sy;
                    camera.pan(dx, dy);
                    pan_start.set((coords.x, coords.y));
                },
                onmouseup: move |_| {
                    is_panning.set(false);
                    if let Some(note_id) = *dragging_note.read() {
                        if let Some(pos) = positions.read().get(&note_id).copied() {
                            if let Some(ref s) = *app_state.read() {
                                s.event_bus.try_publish(
                                    pevents::Event::SetNodePosition { id: note_id, position: pos },
                                );
                            }
                        }
                        dragged_set.write().remove(&note_id);
                    }
                    dragging_note.set(None);
                },
                onmouseleave: move |_| {
                    is_panning.set(false);
                    if let Some(note_id) = *dragging_note.read() {
                        if let Some(pos) = positions.read().get(&note_id).copied() {
                            if let Some(ref s) = *app_state.read() {
                                s.event_bus.try_publish(
                                    pevents::Event::SetNodePosition { id: note_id, position: pos },
                                );
                            }
                        }
                        dragged_set.write().remove(&note_id);
                    }
                    dragging_note.set(None);
                },
                onmousedown: move |evt: Event<MouseData>| {
                    if dragging_note().is_some() {
                        return;
                    }
                    if is_panning() {
                        return;
                    }
                    let coords = evt.data.client_coordinates();
                    is_panning.set(true);
                    pan_start.set((coords.x, coords.y));
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
                    if dz == 0.0 {
                        return;
                    }
                    let coords = evt.data.client_coordinates();
                    camera.zoom_at(dz * -0.001, coords.x, coords.y);
                },

                // Graph canvas (just a drawing surface as events handled by root div)
                canvas {
                    id: "penumbra-graph",
                    style: "position: absolute; inset: 0; width: 100%; height: 100%;",
                }

                GraphCards {
                    app_state,
                    positions,
                    camera,
                    dragging_note,
                    drag_offset,
                    dragged_set,
                    filter_tag: selected_tag(),
                    linking_from,
                    selected,
                    hovered,
                    on_open_editor: {
                        let mut mode = app_mode;
                        move |note_id| {
                            mode.set(AppMode::Editor { note_id: Some(note_id) });
                        }
                    },
                }

                TopBar {}
                FloatingSidebar {
                    app_state,
                    on_note_selected: move |note_id| {
                        selected.set(Some(note_id));
                        camera.drift_zoom(1.0);
                        if let Some(pos) = positions.read().get(&note_id).copied() {
                            camera.drift_to(pos);
                        }
                    },
                    on_tag_filter: {
                        let mut tag = selected_tag;
                        move |t| tag.set(t)
                    },
                    on_open_editor: {
                        let mut mode = app_mode;
                        move |note_id| {
                            mode.set(AppMode::Editor { note_id: Some(note_id) });
                        }
                    },
                }
                Fab {
                    onclick: move |_| create_note(),
                }

                // Zoom / view controls (bottom-left).
                ZoomControls { camera, positions, window_size }

                // Linking banner.
                if linking_from().is_some() {
                    div {
                        style: "position: absolute; top: 64px; left: 50%; transform: translateX(-50%); \
                                 display: flex; align-items: center; gap: 10px; \
                                 background: rgba(255,185,100,0.14); border: 1px solid rgba(255,185,100,0.4); \
                                 color: #ffce8a; padding: 8px 14px; border-radius: 999px; font-size: 13px; \
                                 backdrop-filter: blur(8px); z-index: 600;",
                        Link2 { size: 14, stroke: "currentColor" }
                        span { "Click another note to link  ·  Esc to cancel" }
                        button {
                            style: "background: rgba(255,185,100,0.2); border: none; color: #ffce8a; \
                                     border-radius: 6px; padding: 2px 8px; cursor: pointer; font-size: 12px;",
                            onclick: move |_| linking_from.set(None),
                            "Cancel"
                        }
                    }
                }

                // Empty state.
                if note_count() == 0 && ready() {
                    div {
                        style: "position: absolute; inset: 0; display: flex; flex-direction: column; \
                                 align-items: center; justify-content: center; gap: 14px; \
                                 pointer-events: none; z-index: 50;",
                        div {
                            style: "color: var(--text-dim); font-size: 17px; font-weight: 500;",
                            "Your canvas is empty"
                        }
                        div {
                            style: "color: var(--text-faint); font-size: 13px;",
                            "Press the + button or ⌘N to create your first note"
                        }
                    }
                }
            }
        },
        AppMode::Editor { note_id } => rsx! {
            NoteEditor {
                app_state,
                note_id,
                on_back: move |_| {
                    camera.drift_zoom(1.0);
                    let (w, h) = *window_size.read();
                    camera.drift_to(Position::new(w as f64 / 2.0, h as f64 / 2.0));
                    app_mode.set(AppMode::Graph);
                },
                on_delete: move |_| {
                    if let Some(nid) = note_id {
                        let app = app_state;
                        spawn(async move {
                            if let Some(ref s) = *app.read() {
                                if let Err(e) = s.remove_note(&nid).await {
                                    tracing::error!("remove_note: {e}");
                                }
                            }
                        });
                    }
                    app_mode.set(AppMode::Graph);
                },
            }
        },
    }
}

/// Floating zoom / fit-to-view controls (bottom-left of the canvas).
#[allow(non_snake_case)]
#[component]
fn ZoomControls(
    camera: CameraHandle,
    positions: Signal<HashMap<NoteId, Position>>,
    window_size: Signal<(f32, f32)>,
) -> Element {
    let mut camera = camera;
    let zoom = *camera.zoom.read();
    let pct = (zoom * 100.0).round() as i64;

    let btn = "width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; \
               background: transparent; border: none; color: #7abaff; cursor: pointer; border-radius: 8px; \
               transition: background 120ms;";

    rsx! {
        style { {ZOOM_CSS} }
        div { class: "pnb-zoom",
            button {
                class: "pnb-zoom-btn",
                style: "{btn}",
                title: "Zoom in",
                onclick: move |_| {
                    let (w, h) = *window_size.read();
                    camera.zoom_at(0.2, w as f64 / 2.0, h as f64 / 2.0);
                },
                Plus { size: 16, stroke: "currentColor" }
            }
            div { class: "pnb-zoom-pct", "{pct}%" }
            button {
                class: "pnb-zoom-btn",
                style: "{btn}",
                title: "Zoom out",
                onclick: move |_| {
                    let (w, h) = *window_size.read();
                    camera.zoom_at(-0.2, w as f64 / 2.0, h as f64 / 2.0);
                },
                Minus { size: 16, stroke: "currentColor" }
            }
            div { style: "height: 1px; background: rgba(99,148,220,0.15); margin: 2px 6px;" }
            button {
                class: "pnb-zoom-btn",
                style: "{btn}",
                title: "Fit all notes",
                onclick: move |_| {
                    let pos = positions.read();
                    if pos.is_empty() {
                        return;
                    }
                    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
                    let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
                    for p in pos.values() {
                        min_x = min_x.min(p.x);
                        min_y = min_y.min(p.y);
                        max_x = max_x.max(p.x);
                        max_y = max_y.max(p.y);
                    }
                    let (w, h) = *window_size.read();
                    let (w, h) = (w as f64, h as f64);
                    let span_x = (max_x - min_x).max(1.0) + 320.0;
                    let span_y = (max_y - min_y).max(1.0) + 320.0;
                    let target_zoom = (w / span_x).min(h / span_y).clamp(0.1, 2.0);
                    let center = Position::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
                    // Set zoom directly, then center the camera on the bounding box midpoint.
                    *camera.zoom.write() = target_zoom;
                    *camera.x.write() = w / 2.0 - center.x * target_zoom;
                    *camera.y.write() = h / 2.0 - center.y * target_zoom;
                    camera.cancel_drift();
                },
                Maximize { size: 15, stroke: "currentColor" }
            }
        }
    }
}

const ZOOM_CSS: &str = r#"
.pnb-zoom {
    position: absolute;
    bottom: 16px;
    left: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 5px;
    background: var(--bg-elevated);
    border: 0.5px solid var(--border);
    border-radius: 12px;
    z-index: 600;
    box-shadow: var(--shadow-sm);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
}
.pnb-zoom-btn { color: var(--accent-bright) !important; }
.pnb-zoom-btn:hover { background: var(--accent-soft) !important; }
.pnb-zoom-pct {
    font-size: 10px;
    color: var(--text-dim);
    padding: 1px 0;
    user-select: none;
}
"#;
