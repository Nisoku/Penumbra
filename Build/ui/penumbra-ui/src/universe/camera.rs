use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;

use crate::app_state::{
    ease_in_out_cubic, SharedState, CameraGlide, FIT_MARGIN, FIT_MAX_ZOOM, FIT_MIN_ZOOM,
    GLIDE_DURATION, GLIDE_EPSILON, GLIDE_TICK, TOP_BAR_HEIGHT,
};
use crate::editor_session::maybe_reveal_editor;
use crate::map::{self, MapCamera};
use crate::platform::Instant;
use crate::AppWindow;

use super::rebuild_paths;

/// Fit every card into view
pub(crate) fn center_camera_on_cards(state: &SharedState, ui: &AppWindow) {
    let cards = state.cards.lock().unwrap();
    let Some(first) = cards.first() else {
        return;
    };
    let mut min_x = first.x;
    let mut max_x = first.x + map::CARD_W;
    let mut min_y = first.y;
    let mut max_y = first.y + map::CARD_H;
    for card in cards.iter().skip(1) {
        min_x = min_x.min(card.x);
        max_x = max_x.max(card.x + map::CARD_W);
        min_y = min_y.min(card.y);
        max_y = max_y.max(card.y + map::CARD_H);
    }
    drop(cards);

    let (view_w, view_h) = viewport_size(ui);
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);

    let fit = (view_w / span_x).min(view_h / span_y) * FIT_MARGIN;
    let zoom = fit.clamp(FIT_MIN_ZOOM, FIT_MAX_ZOOM);
    ui.set_zoom(zoom);

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    ui.set_camera_x(center_x - view_w * 0.5 / zoom);
    ui.set_camera_y(center_y - view_h * 0.5 / zoom);
}

/// The drawable map area in logical pixels, excluding the top bar.
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
