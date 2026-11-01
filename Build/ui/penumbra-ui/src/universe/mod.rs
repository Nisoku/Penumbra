mod camera;
mod linking;
mod physics;
mod search;

use std::rc::Rc;
use std::sync::atomic::Ordering;

use slint::ComponentHandle;

use crate::app_state::{
    apply_card_visual_model, set_status, PendingLink, SharedState, UI_CARDS_MODEL,
};
use crate::map::{self, MapCamera};
use crate::{platform, AppWindow, NoteCardVM, TagVM};

pub(crate) use camera::{
    cancel_camera_glide, center_camera_on_cards, start_camera_glide, start_glide, viewport_size,
};
pub(crate) use physics::{publish_drift, start_ambient_settle};
pub(crate) use search::wire_search;

use linking::{create_manual_link, pin_note, settle_note, unpin_note};
use physics::cancel_ambient_settle;

pub(crate) fn wire_map(ui: &AppWindow, state: &Rc<SharedState>) {
    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_camera_changed(move |_, _, _| {
            cancel_camera_glide(&state);
            state.user_panned.store(true, Ordering::Release);
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_selected(move |card_id| {
            state.selected.store(card_id, Ordering::Relaxed);
            cancel_ambient_settle(&state);
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_moved(move |card_id, x, y| {
            state.dirty.store(true, Ordering::Relaxed);
            apply_card_visual(&state, card_id, x, y);
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_released(move |card_id, x, y| {
            let pin_pos = state.pins.lock().unwrap().get(&card_id).copied();
            if let Some(pin_pos) = pin_pos {
                let Some(ui) = handle.upgrade() else {
                    return;
                };
                publish_drift(&state, &ui, vec![(card_id, pin_pos)]);
                return;
            }
            let state = Rc::clone(&state);
            let handle = handle.clone();
            platform::spawn(async move {
                if let Some(settled) = settle_note(state.clone(), card_id, (x, y)).await {
                    if let Some(ui) = handle.upgrade() {
                        publish_drift(&state, &ui, settled);
                    }
                }
            });
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_pinned(move |card_id, x, y| {
            state.dirty.store(true, Ordering::Relaxed);
            set_card_pinned(&state, card_id, true, (x, y));
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
            let state = Rc::clone(&state);
            let handle = handle.clone();
            platform::spawn(async move {
                if let Some(settled) = pin_note(state.clone(), card_id, (x, y)).await {
                    if let Some(ui) = handle.upgrade() {
                        publish_drift(&state, &ui, settled);
                    }
                }
            });
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_unpinned(move |card_id| {
            state.dirty.store(true, Ordering::Relaxed);
            set_card_pinned(&state, card_id, false, (0.0, 0.0));
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
            let state = Rc::clone(&state);
            let handle = handle.clone();
            platform::spawn(async move {
                if let Some(settled) = unpin_note(state.clone(), card_id).await {
                    if let Some(ui) = handle.upgrade() {
                        publish_drift(&state, &ui, settled);
                    }
                }
            });
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_link_start(move |card_id| {
            cancel_ambient_settle(&state);
            *state.pending_link.lock().unwrap() = Some(PendingLink {
                source_card: card_id,
                pointer: (0.0, 0.0),
            });
            if let Some(ui) = handle.upgrade() {
                set_status(&ui.as_weak(), "drag to another note to link");
            }
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_link_move(move |_card_id, wx, wy| {
            if let Some(link) = state.pending_link.lock().unwrap().as_mut() {
                link.pointer = (wx, wy);
            }
            if let Some(ui) = handle.upgrade() {
                rebuild_paths(&state, &ui);
            }
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_link_end(move |source_card, wx, wy| {
            let pending = state.pending_link.lock().unwrap().take();
            let Some(ui) = handle.upgrade() else {
                return;
            };
            ui.set_pending_link_path(Default::default());
            rebuild_paths(&state, &ui);
            if pending.is_none() {
                return;
            }
            let target = {
                let cards = state.cards.lock().unwrap();
                map::card_at_world(&cards, wx, wy)
            };
            let Some(target_card) = target else {
                set_status(&ui.as_weak(), "link cancelled");
                return;
            };
            if target_card == source_card {
                set_status(&ui.as_weak(), "a note cannot link to itself");
                return;
            }
            let state = Rc::clone(&state);
            let handle = handle.clone();
            platform::spawn(async move {
                create_manual_link(state, handle, source_card, target_card).await;
            });
        });
    }
}

pub(crate) fn publish_visuals(
    state: &SharedState,
    ui: &AppWindow,
    cards: Vec<NoteCardVM>,
    tags: Vec<Vec<TagVM>>,
    links: Vec<(i32, i32)>,
) {
    let model = Rc::new(slint::VecModel::from(cards.clone()));
    *state.cards.lock().unwrap() = cards;
    *state.links.lock().unwrap() = links;
    ui.set_note_cards(slint::ModelRc::new(Rc::clone(&model)));
    UI_CARDS_MODEL.with(|cell| {
        *cell.borrow_mut() = Some(Rc::clone(&model));
    });
    set_note_tags(ui, tags);
    rebuild_paths(state, ui);
}

pub(crate) fn set_note_tags(ui: &AppWindow, tags: Vec<Vec<TagVM>>) {
    let rows: Vec<slint::ModelRc<TagVM>> = tags
        .into_iter()
        .map(|row| slint::ModelRc::new(Rc::new(slint::VecModel::from(row))))
        .collect();
    ui.set_note_tags(slint::ModelRc::new(Rc::new(slint::VecModel::from(rows))));
}

/// Update one card's position in the shared state and the live Slint model.
pub(super) fn apply_card_visual(state: &SharedState, card_id: i32, x: f32, y: f32) {
    let mut cards = state.cards.lock().unwrap();
    let Some(idx) = cards.iter().position(|card| card.id == card_id) else {
        return;
    };
    let mut row = cards[idx].clone();
    row.x = x;
    row.y = y;
    cards[idx] = row.clone();
    drop(cards);
    apply_card_visual_model(idx, row);
}

/// Toggle a card's pinned state, recording the pin anchor and syncing the
/// shared pin map, card state, and live Slint model.
fn set_card_pinned(state: &SharedState, card_id: i32, pinned: bool, pos: (f32, f32)) {
    let mut pins = state.pins.lock().unwrap();
    if pinned {
        pins.insert(card_id, pos);
    } else {
        pins.remove(&card_id);
    }
    drop(pins);

    let mut cards = state.cards.lock().unwrap();
    let Some(idx) = cards.iter().position(|card| card.id == card_id) else {
        return;
    };
    let mut row = cards[idx].clone();
    row.pinned = pinned;
    cards[idx] = row.clone();
    drop(cards);
    apply_card_visual_model(idx, row);
}

/// Rebuild every derived map path (grid, edges, pending link) from the current
/// camera and card/link state.
pub(crate) fn rebuild_paths(state: &SharedState, ui: &AppWindow) {
    let cam = MapCamera {
        x: ui.get_camera_x(),
        y: ui.get_camera_y(),
        zoom: ui.get_zoom(),
    };
    let (view_w, view_h) = viewport_size(ui);

    let cards = state.cards.lock().unwrap().clone();
    let links = state.links.lock().unwrap().clone();
    let selected = state.selected.load(Ordering::Relaxed);
    let edges = map::build_edges(&cards, &links, cam, view_w, view_h, selected);
    let grid = map::build_grid(cam, view_w, view_h, map::GRID_SPACING);
    ui.set_grid_path(grid.into());
    ui.set_edge_path(edges.base.into());
    ui.set_selected_edge_path(edges.selected.into());

    let pending = {
        let guard = state.pending_link.lock().unwrap();
        guard.as_ref().and_then(|link| {
            cards
                .iter()
                .find(|card| card.id == link.source_card)
                .map(|card| {
                    map::build_pending_link(card.x, card.y, link.pointer.0, link.pointer.1, cam)
                })
        })
    };
    ui.set_pending_link_path(pending.unwrap_or_default().into());
}
