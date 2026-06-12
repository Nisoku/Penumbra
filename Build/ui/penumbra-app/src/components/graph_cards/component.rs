use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_motion::prelude::*;

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

use crate::components::note_card::NoteCard;
use crate::hooks::CameraHandle;
use crate::state::AppState;

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
         pointer-events: none; overflow: visible;"
    );

    let mut ctx_menu: Signal<Option<CtxMenu>> = use_signal(|| None);

    let ctx_menu_css = concat!(
        ".ctx-menu-item:hover{background:rgba(99,148,220,0.12)}",
        ".ctx-menu-item-delete:hover{background:rgba(255,107,107,0.12)}",
    );
    rsx! {
        style { "{ctx_menu_css}" }
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
                        pinned: note.meta.pinned,
                        camera,
                        dragging_note,
                        drag_offset,
                        dragged_set,
                        on_context_menu: {
                            let mut ctx = ctx_menu;
                            move |(screen_x, screen_y, note_id, pinned): (f64, f64, NoteId, bool)| {
                                ctx.set(Some(CtxMenu {
                                    screen_x,
                                    screen_y,
                                    note_id,
                                    pinned,
                                }));
                            }
                        },
                    }
                }
            }
        }
        // Context menu overlay
        if let Some(ref m) = *ctx_menu.read() {
            div {
                style: "position: fixed; inset: 0; z-index: 999;",
                onclick: move |_| ctx_menu.set(None),
                oncontextmenu: move |evt: Event<MouseData>| evt.prevent_default(),
                div {
                    style: "position: absolute; left: {m.screen_x}px; top: {m.screen_y}px; \
                             min-width: 160px; \
                             background: rgba(15, 23, 42, 0.95); \
                             border: 1px solid rgba(99, 148, 220, 0.25); \
                             border-radius: 8px; \
                             padding: 4px; \
                             box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5); \
                             z-index: 1000;",
                    onclick: move |evt: Event<MouseData>| evt.stop_propagation(),
                    // Open in editor
                    div {
                        class: "ctx-menu-item",
                        style: "padding: 8px 12px; border-radius: 4px; cursor: pointer; \
                                 color: #c8e3ff; font-size: 13px;",
                        onclick: {
                            let cb = on_open_editor;
                            let mut ctx = ctx_menu;
                            move |_| {
                                let m = match &*ctx.read() {
                                    Some(m) => m.note_id,
                                    None => return,
                                };
                                ctx.set(None);
                                cb.call(m);
                            }
                        },
                        "Open in editor"
                    }
                    // Pin / Unpin
                    div {
                        class: "ctx-menu-item",
                        style: "padding: 8px 12px; border-radius: 4px; cursor: pointer; \
                                 color: #c8e3ff; font-size: 13px;",
                        onclick: {
                            let mut ctx = ctx_menu;
                            let app = app_state;
                            move |_| {
                                let id = match &*ctx.read() {
                                    Some(m) => m.note_id,
                                    None => return,
                                };
                                ctx.set(None);
                                let app = app;
                                spawn(async move {
                                    if let Some(ref s) = *app.read() {
                                        let _ = s.toggle_pin(&id).await;
                                    }
                                });
                            }
                        },
                        if m.pinned { "Unpin" } else { "Pin to canvas" }
                    }
                    // Delete
                    div {
                        class: "ctx-menu-item-delete",
                        style: "padding: 8px 12px; border-radius: 4px; cursor: pointer; \
                                 color: #ff6b6b; font-size: 13px;",
                        onclick: {
                            let mut ctx = ctx_menu;
                            let app = app_state;
                            move |_| {
                                let id = match &*ctx.read() {
                                    Some(m) => m.note_id,
                                    None => return,
                                };
                                ctx.set(None);
                                let app = app;
                                spawn(async move {
                                    if let Some(ref s) = *app.read() {
                                        let _ = s.remove_note(&id).await;
                                    }
                                });
                            }
                        },
                        "Delete note"
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
    pinned: bool,
    camera: CameraHandle,
    dragging_note: Signal<Option<NoteId>>,
    drag_offset: Signal<(f64, f64)>,
    dragged_set: Signal<std::collections::HashSet<NoteId>>,
    on_context_menu: EventHandler<(f64, f64, NoteId, bool)>,
) -> Element {
    let mut spring = use_motion(0.0f32);
    let mut anim_x = use_motion(x as f32);
    let mut anim_y = use_motion(y as f32);

    let mut target_x = use_signal(|| x);
    let mut target_y = use_signal(|| y);

    if (target_x() - x).abs() > 0.001 {
        target_x.set(x);
    }
    if (target_y() - y).abs() > 0.001 {
        target_y.set(y);
    }

    let spring_cfg = AnimationConfig::new(AnimationMode::Spring(Spring {
        stiffness: 180.0,
        damping: 18.0,
        ..Default::default()
    }));
    let spring_cfg_2 = spring_cfg.clone();

    use_effect(move || {
        spring.animate_to(1.0, spring_cfg.clone());
    });

    use_effect(move || {
        let tx = target_x();
        let ty = target_y();
        anim_x.animate_to(tx as f32, spring_cfg_2.clone());
        anim_y.animate_to(ty as f32, spring_cfg_2.clone());
    });

    let s = spring.get_value() as f64;
    let px = anim_x.get_value() as f64;
    let py = anim_y.get_value() as f64;

    rsx! {
        div {
            style: "position: absolute; transform: translate({px}px, {py}px); \
                     pointer-events: auto;",
            oncontextmenu: move |evt: Event<MouseData>| {
                evt.prevent_default();
                evt.stop_propagation();
                let coords = evt.data.client_coordinates();
                on_context_menu.call((coords.x, coords.y, id, pinned));
            },
            onmousedown: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                dragged_set.write().insert(id);
                let coords = evt.data.client_coordinates();
                let zoom = *camera.zoom.read();
                let mx = (coords.x - *camera.x.read()) / zoom;
                let my = (coords.y - *camera.y.read()) / zoom;
                drag_offset.set((mx - px, my - py));
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
