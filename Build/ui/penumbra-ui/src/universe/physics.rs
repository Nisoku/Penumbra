use std::rc::Rc;
use std::sync::atomic::Ordering;

use slint::ComponentHandle;

use crate::app_state::{
    note_code, Drift, DriftTarget, SharedState, DRIFT_DURATION, DRIFT_TICK, SETTLE_CONVERGENCE,
    SETTLE_MAX_FRAMES, SETTLE_STEPS_PER_FRAME, SETTLE_TICK,
};
use crate::platform::{self, Instant};
use crate::AppWindow;

use super::camera::center_camera_on_cards;
use super::{apply_card_visual, rebuild_paths};

pub(crate) fn start_ambient_settle(state: &Rc<SharedState>, ui: &AppWindow) {
    if ui.get_editor_open() {
        return;
    }
    if state
        .settling
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    arm_settle_tick(state, &ui.as_weak(), 0);
}

pub(crate) fn cancel_ambient_settle(state: &SharedState) {
    state.settling.store(false, Ordering::Release);
}

fn arm_settle_tick(state: &Rc<SharedState>, handle: &slint::Weak<AppWindow>, frame: u32) {
    let state = Rc::clone(state);
    let handle = handle.clone();
    slint::Timer::single_shot(SETTLE_TICK, move || {
        settle_frame(state, handle, frame);
    });
}

fn settle_frame(state: Rc<SharedState>, handle: slint::Weak<AppWindow>, frame: u32) {
    if !state.settling.load(Ordering::Acquire) {
        return;
    }
    let task_state = Rc::clone(&state);
    platform::spawn(async move {
        if !task_state.settling.load(Ordering::Acquire) {
            return;
        }
        let (disp, settled) = {
            let mut layout_guard = task_state.layout.lock().await;
            let Some(layout) = layout_guard.as_mut() else {
                task_state.settling.store(false, Ordering::Release);
                return;
            };
            let mut disp = 0.0f64;
            for _ in 0..SETTLE_STEPS_PER_FRAME {
                disp = layout.step();
            }
            (disp, settled_positions(layout))
        };

        let pins = task_state.pins.lock().unwrap();
        for (card_id, (sx, sy)) in &settled {
            if pins.contains_key(card_id) {
                continue;
            }
            apply_card_visual(&task_state, *card_id, *sx, *sy);
        }
        drop(pins);
        task_state.dirty.store(true, Ordering::Relaxed);

        let Some(ui) = handle.upgrade() else {
            task_state.settling.store(false, Ordering::Release);
            return;
        };
        rebuild_paths(&task_state, &ui);

        let converged = disp < SETTLE_CONVERGENCE;
        if converged || frame + 1 >= SETTLE_MAX_FRAMES {
            task_state.settling.store(false, Ordering::Release);
            if !task_state.user_panned.load(Ordering::Acquire) {
                center_camera_on_cards(&task_state, &ui);
                rebuild_paths(&task_state, &ui);
            }
        } else {
            arm_settle_tick(&task_state, &handle, frame + 1);
        }
    });
}

pub(crate) fn settled_positions(layout: &penumbra_layout::LayoutEngine) -> Vec<(i32, (f32, f32))> {
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
        let progress = crate::app_state::ease_in_out_cubic((elapsed / total).min(1.0));
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
