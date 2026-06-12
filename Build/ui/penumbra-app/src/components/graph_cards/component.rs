use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus::html::input_data::MouseButton;
use dioxus_icons::lucide::{Link2, Pencil, Pin, PinOff, Trash2, Unlink};

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

use crate::components::note_card::NoteCard;
use crate::hooks::CameraHandle;
use crate::state::AppState;

// Below this zoom level the canvas LOD dots take over; hide HTML cards.
const LOD_ZOOM_THRESHOLD: f64 = 0.45;

#[derive(Debug, Clone, PartialEq)]
struct CtxMenu {
    screen_x: f64,
    screen_y: f64,
    note_id: NoteId,
    pinned: bool,
}

#[allow(non_snake_case)]
#[component]
pub fn GraphCards(
    app_state: Signal<Option<Arc<AppState>>>,
    positions: Signal<HashMap<NoteId, Position>>,
    camera: CameraHandle,
    dragging_note: Signal<Option<NoteId>>,
    drag_offset: Signal<(f64, f64)>,
    dragged_set: Signal<std::collections::HashSet<NoteId>>,
    filter_tag: Option<String>,
    linking_from: Signal<Option<NoteId>>,
    selected: Signal<Option<NoteId>>,
    hovered: Signal<Option<NoteId>>,
    on_open_editor: EventHandler<NoteId>,
) -> Element {
    let cx = *camera.x.read();
    let cy = *camera.y.read();
    let zoom = *camera.zoom.read();

    let notes = match &*app_state.read() {
        Some(state) => {
            let g = state.graph.lock().expect("graph lock poisoned");
            let all: Vec<_> = g.all_notes().cloned().collect();
            if let Some(ref tag) = filter_tag {
                all.into_iter().filter(|n| n.tags.contains(tag)).collect()
            } else {
                all
            }
        }
        None => Vec::new(),
    };

    let container_style = format!(
        "transform: translate({cx}px, {cy}px) scale({zoom}); \
         transform-origin: 0 0; \
         position: absolute; inset: 0; \
         overflow: visible;"
    );

    let mut ctx_menu: Signal<Option<CtxMenu>> = use_signal(|| None);

    // Only render HTML cards above the LOD threshold.
    let show_cards = zoom >= LOD_ZOOM_THRESHOLD;
    let link_src = linking_from();

    // Does a link already exist between the menu target and the link source?
    let menu_already_linked = {
        if let (Some(m), Some(src)) = (ctx_menu.read().clone(), link_src) {
            app_state
                .read()
                .as_ref()
                .map(|s| s.are_linked(&src, &m.note_id))
                .unwrap_or(false)
        } else {
            false
        }
    };

    rsx! {
        // Context-menu hover styles (CSS, not eval, works reliably).
        style { {CTX_MENU_CSS} }

        div {
            style: "{container_style}",
            if show_cards {
                for note in &notes {
                    if let Some(pos) = positions.read().get(&note.id).copied() {
                        AnimatedCard {
                            key: "{note.id}",
                            title: note.title.clone(),
                            preview: note.body.clone(),
                            tags: note.tags.clone(),
                            x: pos.x,
                            y: pos.y,
                            id: note.id,
                            pinned: note.meta.pinned,
                            camera,
                            dragging_note,
                            drag_offset,
                            dragged_set,
                            linking_from,
                            selected,
                            hovered,
                            on_open_editor,
                            on_finish_link: {
                                let app = app_state;
                                let mut linking = linking_from;
                                move |target: NoteId| {
                                    let Some(src) = linking() else { return };
                                    linking.set(None);
                                    spawn(async move {
                                        if let Some(ref s) = *app.read() {
                                            let _ = s.link_and_save(&src, &target).await;
                                        }
                                    });
                                }
                            },
                            on_context_menu: {
                                let mut ctx = ctx_menu;
                                move |(screen_x, screen_y, note_id, pinned): (f64, f64, NoteId, bool)| {
                                    ctx.set(Some(CtxMenu { screen_x, screen_y, note_id, pinned }));
                                }
                            },
                        }
                    }
                }
            }
        }

        // Context menu overlay always on top regardless of LOD.
        if let Some(m) = ctx_menu.read().clone() {
            div {
                style: "position: fixed; inset: 0; z-index: 999;",
                onclick: move |_| ctx_menu.set(None),
                oncontextmenu: move |evt: Event<MouseData>| evt.prevent_default(),
                div {
                    class: "pnb-ctx-menu",
                    style: "left: {m.screen_x}px; top: {m.screen_y}px;",
                    onclick: move |evt: Event<MouseData>| evt.stop_propagation(),

                    div {
                        class: "pnb-ctx-item",
                        onclick: {
                            let cb = on_open_editor;
                            let mut ctx = ctx_menu;
                            move |_| { let id = m.note_id; ctx.set(None); cb.call(id); }
                        },
                        Pencil { size: 14, stroke: "currentColor" }
                        span { "Open in editor" }
                    }

                    div {
                        class: "pnb-ctx-item",
                        onclick: {
                            let mut ctx = ctx_menu;
                            let app = app_state;
                            move |_| {
                                let id = m.note_id;
                                ctx.set(None);
                                spawn(async move {
                                    if let Some(ref s) = *app.read() { let _ = s.toggle_pin(&id).await; }
                                });
                            }
                        },
                        if m.pinned {
                            PinOff { size: 14, stroke: "currentColor" }
                            span { "Unpin from canvas" }
                        } else {
                            Pin { size: 14, stroke: "currentColor" }
                            span { "Pin to canvas" }
                        }
                    }

                    div { class: "pnb-ctx-sep" }

                    // Linking actions (depend on current linking state).
                    match link_src {
                        Some(src) if src == m.note_id => rsx! {
                            div {
                                class: "pnb-ctx-item",
                                onclick: {
                                    let mut ctx = ctx_menu;
                                    let mut linking = linking_from;
                                    move |_| { linking.set(None); ctx.set(None); }
                                },
                                Unlink { size: 14, stroke: "currentColor" }
                                span { "Cancel linking" }
                            }
                        },
                        Some(src) if menu_already_linked => rsx! {
                            div {
                                class: "pnb-ctx-item",
                                onclick: {
                                    let mut ctx = ctx_menu;
                                    let mut linking = linking_from;
                                    let app = app_state;
                                    let tgt = m.note_id;
                                    move |_| {
                                        linking.set(None);
                                        ctx.set(None);
                                        spawn(async move {
                                            if let Some(ref s) = *app.read() {
                                                let _ = s.unlink_and_save(&src, &tgt).await;
                                            }
                                        });
                                    }
                                },
                                Unlink { size: 14, stroke: "currentColor" }
                                span { "Unlink these notes" }
                            }
                        },
                        Some(src) => rsx! {
                            div {
                                class: "pnb-ctx-item",
                                onclick: {
                                    let mut ctx = ctx_menu;
                                    let mut linking = linking_from;
                                    let app = app_state;
                                    let tgt = m.note_id;
                                    move |_| {
                                        linking.set(None);
                                        ctx.set(None);
                                        spawn(async move {
                                            if let Some(ref s) = *app.read() {
                                                let _ = s.link_and_save(&src, &tgt).await;
                                            }
                                        });
                                    }
                                },
                                Link2 { size: 14, stroke: "currentColor" }
                                span { "Link to this note" }
                            }
                        },
                        None => rsx! {
                            div {
                                class: "pnb-ctx-item",
                                onclick: {
                                    let mut ctx = ctx_menu;
                                    let mut linking = linking_from;
                                    move |_| { linking.set(Some(m.note_id)); ctx.set(None); }
                                },
                                Link2 { size: 14, stroke: "currentColor" }
                                span { "Link from here…" }
                            }
                        },
                    }

                    div { class: "pnb-ctx-sep" }

                    div {
                        class: "pnb-ctx-item danger",
                        onclick: {
                            let mut ctx = ctx_menu;
                            let app = app_state;
                            move |_| {
                                let id = m.note_id;
                                ctx.set(None);
                                spawn(async move {
                                    if let Some(ref s) = *app.read() { let _ = s.remove_note(&id).await; }
                                });
                            }
                        },
                        Trash2 { size: 14, stroke: "currentColor" }
                        span { "Delete note" }
                    }
                }
            }
        }
    }
}

const CTX_MENU_CSS: &str = r#"
.pnb-ctx-menu {
    position: absolute;
    min-width: 184px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 5px;
    box-shadow: var(--shadow-lg);
    z-index: 1000;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    animation: pnb-menu-in 120ms ease-out;
}
@keyframes pnb-menu-in {
    from { opacity: 0; transform: scale(0.97); }
    to   { opacity: 1; transform: scale(1); }
}
.pnb-ctx-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 11px;
    border-radius: 6px;
    cursor: pointer;
    color: var(--text);
    font-size: 13px;
    transition: background 120ms ease;
}
.pnb-ctx-item:hover { background: var(--accent-soft); }
.pnb-ctx-item.danger { color: var(--danger); }
.pnb-ctx-item.danger:hover { background: var(--danger-soft); }
.pnb-ctx-sep { height: 1px; background: var(--border); margin: 4px 8px; }

@keyframes pnb-card-in {
    from { opacity: 0; transform: translateY(4px) scale(0.96); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
}
.pnb-card-enter > div { animation: pnb-card-in 220ms cubic-bezier(0.22, 1, 0.36, 1); }
"#;

// Animated card.
//
// Position comes straight from the physics engine (the `positions` signal),
// which already moves smoothly at ~60fps so we DON'T add a second position
// spring on top
#[allow(non_snake_case)]
#[component]
fn AnimatedCard(
    title: String,
    preview: String,
    tags: Vec<String>,
    x: f64,
    y: f64,
    id: NoteId,
    pinned: bool,
    camera: CameraHandle,
    dragging_note: Signal<Option<NoteId>>,
    drag_offset: Signal<(f64, f64)>,
    dragged_set: Signal<std::collections::HashSet<NoteId>>,
    linking_from: Signal<Option<NoteId>>,
    selected: Signal<Option<NoteId>>,
    hovered: Signal<Option<NoteId>>,
    on_open_editor: EventHandler<NoteId>,
    on_finish_link: EventHandler<NoteId>,
    on_context_menu: EventHandler<(f64, f64, NoteId, bool)>,
) -> Element {
    let is_dragging = dragging_note().map(|d| d == id).unwrap_or(false);

    let link_src = linking_from();
    let is_link_source = link_src == Some(id);
    let linking_active = link_src.is_some();
    let is_selected = selected() == Some(id);

    let z_idx = if is_dragging { 100usize } else { 1 };
    let drag_shadow = if is_dragging {
        "filter: drop-shadow(0 10px 28px rgba(0,0,0,0.55));"
    } else {
        ""
    };

    // Outline cue for selection / linking.
    let outline = if is_link_source {
        "outline: 2px solid var(--pin); outline-offset: 3px;"
    } else if linking_active {
        "outline: 1.5px dashed var(--accent-bright); outline-offset: 3px; cursor: crosshair;"
    } else if is_selected {
        "outline: 2px solid var(--accent-bright); outline-offset: 3px;"
    } else {
        ""
    };

    rsx! {
        div {
            class: "pnb-card-enter",
            style: "position: absolute; transform: translate({x}px, {y}px); pointer-events: auto; z-index: {z_idx}; border-radius: 10px; {outline} {drag_shadow}",
            ondoubleclick: move |_| on_open_editor.call(id),
            onmouseenter: move |_| { let mut h = hovered; h.set(Some(id)); },
            onmouseleave: move |_| {
                let mut h = hovered;
                if h() == Some(id) { h.set(None); }
            },
            onclick: move |evt: Event<MouseData>| {
                // In linking mode a plain click on another card finishes the link.
                if linking_active && !is_link_source {
                    evt.stop_propagation();
                    on_finish_link.call(id);
                }
            },
            oncontextmenu: move |evt: Event<MouseData>| {
                evt.prevent_default();
                evt.stop_propagation();
                let coords = evt.data.client_coordinates();
                on_context_menu.call((coords.x, coords.y, id, pinned));
            },
            onmousedown: move |evt: Event<MouseData>| {
                if evt.data.held_buttons().contains(MouseButton::Secondary) {
                    evt.stop_propagation();
                    evt.prevent_default();
                    let coords = evt.data.client_coordinates();
                    on_context_menu.call((coords.x, coords.y, id, pinned));
                    return;
                }
                if linking_active {
                    return;
                }
                evt.stop_propagation();
                dragged_set.write().insert(id);
                let coords = evt.data.client_coordinates();
                let zoom = *camera.zoom.read();
                let mx = (coords.x - *camera.x.read()) / zoom;
                let my = (coords.y - *camera.y.read()) / zoom;
                drag_offset.set((mx - x, my - y));
                dragging_note.set(Some(id));
            },
            div {
                style: "pointer-events: none;",
                NoteCard {
                    title,
                    preview,
                    tags,
                    pinned,
                    x: 0.0,
                    y: 0.0,
                }
            }
        }
    }
}
