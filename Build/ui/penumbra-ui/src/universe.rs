use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use penumbra_core::position::Position;

use crate::app_state::{
    apply_card_visual_model, ease_in_out_cubic, note_code, CameraGlide, Drift, DriftTarget,
    SharedState, DRIFT_DURATION, DRIFT_TICK, GLIDE_DURATION, GLIDE_EPSILON, GLIDE_TICK,
    TOP_BAR_HEIGHT, UI_CARDS_MODEL,
};
use crate::editor_session::maybe_reveal_editor;
use crate::map::{self, MapCamera};
use crate::platform::Instant;
use crate::{platform, AppWindow, NoteCardVM};
use slint::ComponentHandle;

/// Wire the map callbacks
pub(crate) fn wire_map(ui: &AppWindow, state: &Rc<SharedState>) {
    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_camera_changed(move |_, _, _| {
            cancel_camera_glide(&state);
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
}

/// Replace the card model and link set, then refresh the derived edge paths.
pub(crate) fn publish_visuals(
    state: &SharedState,
    ui: &AppWindow,
    cards: Vec<NoteCardVM>,
    links: Vec<(i32, i32)>,
) {
    let model = Rc::new(slint::VecModel::from(cards.clone()));
    *state.cards.lock().unwrap() = cards;
    *state.links.lock().unwrap() = links;
    ui.set_note_cards(slint::ModelRc::new(Rc::clone(&model)));
    UI_CARDS_MODEL.with(|cell| {
        *cell.borrow_mut() = Some(Rc::clone(&model));
    });
    rebuild_paths(state, ui);
}

fn apply_card_visual(state: &SharedState, card_id: i32, x: f32, y: f32) {
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

pub(crate) fn center_camera_on_cards(state: &SharedState, ui: &AppWindow) {
    let cards = state.cards.lock().unwrap();
    let Some(first) = cards.first() else {
        return;
    };
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for card in cards.iter().skip(1) {
        min_x = min_x.min(card.x);
        max_x = max_x.max(card.x);
        min_y = min_y.min(card.y);
        max_y = max_y.max(card.y);
    }
    drop(cards);

    let (view_w, view_h) = viewport_size(ui);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    ui.set_camera_x(center_x - view_w * 0.5);
    ui.set_camera_y(center_y - view_h * 0.5);
}

pub(crate) fn viewport_size(ui: &AppWindow) -> (f32, f32) {
    let window = ui.window();
    let size = window.size();
    let scale = window.scale_factor();
    (
        (size.width as f32) / scale,
        ((size.height as f32) / scale - TOP_BAR_HEIGHT).max(0.0),
    )
}

pub(crate) fn cancel_camera_glide(state: &SharedState) {
    *state.glide.lock().unwrap() = None;
}

pub(crate) fn start_camera_glide(state: &Rc<SharedState>, ui: &AppWindow, to: MapCamera) {
    start_glide(state, ui, to, GLIDE_DURATION);
}

pub(crate) fn start_glide(
    state: &Rc<SharedState>,
    ui: &AppWindow,
    to: MapCamera,
    duration: Duration,
) {
    let from = MapCamera {
        x: ui.get_camera_x(),
        y: ui.get_camera_y(),
        zoom: ui.get_zoom(),
    };
    *state.glide.lock().unwrap() = Some(CameraGlide {
        from,
        to,
        started: Instant::now(),
        duration,
    });
    arm_glide_tick(state, ui);
}

fn arm_glide_tick(state: &Rc<SharedState>, ui: &AppWindow) {
    let state = Rc::clone(state);
    let handle = ui.as_weak();
    slint::Timer::single_shot(GLIDE_TICK, move || {
        let Some(ui) = handle.upgrade() else {
            return;
        };
        glide_step(&state, &ui);
    });
}

fn glide_step(state: &Rc<SharedState>, ui: &AppWindow) {
    let finished = {
        let mut guard = state.glide.lock().unwrap();
        let Some(glide) = guard.as_mut() else {
            return;
        };
        let elapsed = glide.started.elapsed().as_secs_f32();
        let total = glide.duration.as_secs_f32();
        let progress = ease_in_out_cubic((elapsed / total).min(1.0));
        let cam = MapCamera {
            x: glide.from.x + (glide.to.x - glide.from.x) * progress,
            y: glide.from.y + (glide.to.y - glide.from.y) * progress,
            zoom: glide.from.zoom + (glide.to.zoom - glide.from.zoom) * progress,
        };
        let delta = (cam.x - glide.to.x).abs()
            + (cam.y - glide.to.y).abs()
            + (cam.zoom - glide.to.zoom).abs();
        if delta < GLIDE_EPSILON {
            ui.set_camera_x(glide.to.x);
            ui.set_camera_y(glide.to.y);
            ui.set_zoom(glide.to.zoom);
            *guard = None;
            true
        } else {
            ui.set_camera_x(cam.x);
            ui.set_camera_y(cam.y);
            ui.set_zoom(cam.zoom);
            elapsed >= total
        }
    };
    rebuild_paths(state, ui);
    if !finished {
        arm_glide_tick(state, ui);
    } else {
        maybe_reveal_editor(state, ui);
    }
}

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
}

async fn settle_note(
    state: Rc<SharedState>,
    card_id: i32,
    pos: (f32, f32),
) -> Option<Vec<(i32, (f32, f32))>> {
    let mut layout_guard = state.layout.lock().await;
    let mut universe_guard = state.universe.lock().await;
    let layout = layout_guard.as_mut()?;
    let universe = universe_guard.as_mut()?;
    let note_id = universe
        .graph()
        .all_notes()
        .map(|note| note.id)
        .find(|note_id| note_code(note_id) == card_id)?;
    layout.set_position(&note_id, Position::new(pos.0 as f64, pos.1 as f64));
    layout.step_neighborhood(&note_id);
    Some(settled_positions(layout))
}

async fn pin_note(
    state: Rc<SharedState>,
    card_id: i32,
    pos: (f32, f32),
) -> Option<Vec<(i32, (f32, f32))>> {
    let mut layout_guard = state.layout.lock().await;
    let mut universe_guard = state.universe.lock().await;
    let layout = layout_guard.as_mut()?;
    let universe = universe_guard.as_mut()?;
    let note_id = universe
        .graph()
        .all_notes()
        .map(|note| note.id)
        .find(|note_id| note_code(note_id) == card_id)?;
    layout.set_position(&note_id, Position::new(pos.0 as f64, pos.1 as f64));
    layout.pin(&note_id, true);
    layout.step_neighborhood(&note_id);
    Some(settled_positions(layout))
}

async fn unpin_note(state: Rc<SharedState>, card_id: i32) -> Option<Vec<(i32, (f32, f32))>> {
    let mut layout_guard = state.layout.lock().await;
    let mut universe_guard = state.universe.lock().await;
    let layout = layout_guard.as_mut()?;
    let universe = universe_guard.as_mut()?;
    let note_id = universe
        .graph()
        .all_notes()
        .map(|note| note.id)
        .find(|note_id| note_code(note_id) == card_id)?;
    layout.pin(&note_id, false);
    layout.step_neighborhood(&note_id);
    Some(settled_positions(layout))
}

/// Snapshot every node's resolved position as card-coded `(id, (x, y))` pairs.
fn settled_positions(layout: &penumbra_layout::LayoutEngine) -> Vec<(i32, (f32, f32))> {
    layout
        .all_positions()
        .iter()
        .map(|(note_id, position)| (note_code(note_id), (position.x as f32, position.y as f32)))
        .collect()
}

pub(crate) fn publish_drift(
    state: &Rc<SharedState>,
    ui: &AppWindow,
    settled: Vec<(i32, (f32, f32))>,
) {
    let cards = state.cards.lock().unwrap();
    let mut items = Vec::new();
    for (card_id, (sx, sy)) in settled {
        let Some(card) = cards.iter().find(|card| card.id == card_id) else {
            continue;
        };
        if (card.x - sx).abs() + (card.y - sy).abs() > 0.25 {
            items.push(DriftTarget {
                card_id,
                from: (card.x, card.y),
                to: (sx, sy),
            });
        }
    }
    drop(cards);
    if items.is_empty() {
        return;
    }
    *state.drift.lock().unwrap() = Some(Drift {
        items,
        started: Instant::now(),
        duration: DRIFT_DURATION,
    });
    arm_drift_tick(state, ui);
}

fn arm_drift_tick(state: &Rc<SharedState>, ui: &AppWindow) {
    let state = Rc::clone(state);
    let handle = ui.as_weak();
    slint::Timer::single_shot(DRIFT_TICK, move || {
        let Some(ui) = handle.upgrade() else {
            return;
        };
        drift_step(&state, &ui);
    });
}

fn drift_step(state: &Rc<SharedState>, ui: &AppWindow) {
    let finished = {
        let mut guard = state.drift.lock().unwrap();
        let Some(drift) = guard.as_mut() else {
            return;
        };
        let elapsed = drift.started.elapsed().as_secs_f32();
        let total = drift.duration.as_secs_f32();
        let progress = ease_in_out_cubic((elapsed / total).min(1.0));
        for target in drift.items.iter() {
            let x = target.from.0 + (target.to.0 - target.from.0) * progress;
            let y = target.from.1 + (target.to.1 - target.from.1) * progress;
            apply_card_visual(state, target.card_id, x, y);
        }
        elapsed >= total
    };
    rebuild_paths(state, ui);
    if finished {
        state.dirty.store(true, Ordering::Relaxed);
        *state.drift.lock().unwrap() = None;
    } else {
        arm_drift_tick(state, ui);
    }
}
