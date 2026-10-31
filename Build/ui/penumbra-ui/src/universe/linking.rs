use std::rc::Rc;

use slint::ComponentHandle;

use penumbra_core::position::Position;

use crate::app_state::{
    build_note_cards, note_code, pinned_note_ids, set_status, SharedState,
};
use crate::AppWindow;

use super::physics::{settled_positions, start_ambient_settle};
use super::publish_visuals;

pub(super) async fn settle_note(
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

pub(super) async fn pin_note(
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

pub(super) async fn unpin_note(
    state: Rc<SharedState>,
    card_id: i32,
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
    layout.pin(&note_id, false);
    layout.step_neighborhood(&note_id);
    Some(settled_positions(layout))
}

pub(super) async fn create_manual_link(
    state: Rc<SharedState>,
    handle: slint::Weak<AppWindow>,
    source_card: i32,
    target_card: i32,
) {
    let updated_note = {
        let mut universe_guard = state.universe.lock().await;
        let Some(universe) = universe_guard.as_mut() else {
            return;
        };

        let (source_id, target_title) = {
            let graph = universe.graph();
            let source_id = graph
                .all_notes()
                .map(|note| note.id)
                .find(|id| note_code(id) == source_card);
            let target_title = graph
                .all_notes()
                .find(|note| note_code(&note.id) == target_card)
                .map(|note| note.title.clone());
            match (source_id, target_title) {
                (Some(id), Some(title)) => (id, title),
                _ => return,
            }
        };

        let mut note = match universe.graph().get_note(&source_id) {
            Some(note) => note.clone(),
            None => return,
        };

        let wikilink = format!("[[{target_title}]]");
        if note.body.contains(&wikilink) {
            set_status(&handle, "already linked");
            return;
        }
        if note.body.trim_end().is_empty() {
            note.body = wikilink;
        } else {
            note.body = format!("{}\n\n{}", note.body.trim_end(), wikilink);
        }
        note.touch();

        if let Err(e) = universe.save_note(note.clone()).await {
            set_status(&handle, &format!("could not link: {e}"));
            return;
        }
        note
    };

    publish_after_link(&state, &handle, updated_note.id).await;
}

async fn publish_after_link(
    state: &Rc<SharedState>,
    handle: &slint::Weak<AppWindow>,
    note_id: penumbra_core::note::NoteId,
) {
    let (cards, links) = {
        let mut layout_guard = state.layout.lock().await;
        let mut universe_guard = state.universe.lock().await;
        let (Some(layout), Some(universe)) = (layout_guard.as_mut(), universe_guard.as_mut())
        else {
            return;
        };

        if let Err(e) = universe
            .storage()
            .save_implicit_links(&universe.implicit_link_pairs())
            .await
        {
            tracing::error!("failed to save implicit links: {e}");
        }

        let graph_links: Vec<_> = universe.graph().all_links().into_iter().cloned().collect();
        layout.update_links(graph_links.clone());
        layout.step_neighborhood(&note_id);

        let pin_codes = state.pins.lock().unwrap().keys().copied().collect();
        let pinned_ids = pinned_note_ids(universe, &pin_codes);
        let cards = build_note_cards(layout, &universe.graph(), &pinned_ids);
        let links: Vec<(i32, i32)> = graph_links
            .iter()
            .map(|link| (note_code(&link.source), note_code(&link.target)))
            .collect();
        (cards, links)
    };

    if let Some(ui) = handle.upgrade() {
        publish_visuals(state, &ui, cards, links);
        set_status(&ui.as_weak(), "linked");
        start_ambient_settle(state, &ui);
    }
}
