use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use slint::ComponentHandle;

use penumbra_core::note::Note;
use penumbra_search::SearchEngine;

use crate::app_state::{note_code, set_status, SharedState};
use crate::map::{self, MapCamera};
use crate::AppWindow;

use super::camera::{start_camera_glide, viewport_size};
use super::rebuild_paths;

pub(crate) fn wire_search(ui: &AppWindow, state: &Rc<SharedState>) {
    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_search_accepted(move |query| {
            let query = query.to_string();
            let state = Rc::clone(&state);
            let handle = handle.clone();
            crate::platform::spawn(async move {
                run_search(state, handle, query).await;
            });
        });
    }
    {
        let handle = ui.as_weak();
        ui.on_search_edited(move |query| {
            if query.is_empty() {
                set_status(&handle, "");
            }
        });
    }
}

async fn run_search(state: Rc<SharedState>, handle: slint::Weak<AppWindow>, query: String) {
    let query = query.trim().to_string();
    if query.is_empty() {
        set_status(&handle, "");
        return;
    }

    let (embedder, index) = {
        let embedder = state.embedder.lock().unwrap().clone();
        let index = state.index.lock().unwrap().clone();
        match (embedder, index) {
            (Some(embedder), Some(index)) => (embedder, index),
            _ => {
                set_status(&handle, "search not ready yet");
                return;
            }
        }
    };

    let notes: Vec<Note> = {
        let guard = state.universe.lock().await;
        let Some(universe) = guard.as_ref() else {
            return;
        };
        let notes = universe.graph().all_notes().cloned().collect();
        notes
    };

    let engine = SearchEngine::new(Arc::clone(&embedder), Arc::clone(&index));
    let results = match engine.search(&query, &notes, &[]).await {
        Ok(results) => results,
        Err(e) => {
            tracing::warn!("search failed: {e}");
            set_status(&handle, "search failed");
            return;
        }
    };

    let Some(top) = results.first() else {
        set_status(&handle, &format!("no match for \u{201c}{query}\u{201d}"));
        return;
    };

    let card_id = note_code(&top.note_id);
    let total = results.len();
    focus_card(&state, &handle, card_id);
    if let Some(ui) = handle.upgrade() {
        let title = top.note.title.as_str();
        let plural = if total == 1 { "match" } else { "matches" };
        ui.set_status(format!("{total} {plural} \u{2014} showing \u{201c}{title}\u{201d}").into());
    }
}

/// Select the card and glide the camera so it sits framed in the viewport.
fn focus_card(state: &Rc<SharedState>, handle: &slint::Weak<AppWindow>, card_id: i32) {
    let Some(ui) = handle.upgrade() else {
        return;
    };

    let pos = state
        .cards
        .lock()
        .unwrap()
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| (card.x, card.y));
    let Some((x, y)) = pos else {
        return;
    };

    state.selected.store(card_id, Ordering::Relaxed);
    ui.set_selected_note(card_id);

    let (view_w, view_h) = viewport_size(&ui);
    let center = MapCamera {
        x: x + map::CARD_W * 0.5 - view_w * 0.5,
        y: y + map::CARD_H * 0.5 - view_h * 0.5,
        zoom: ui.get_zoom(),
    };
    start_camera_glide(state, &ui, center);
    rebuild_paths(state, &ui);
}
