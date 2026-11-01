use std::rc::Rc;
use std::sync::atomic::Ordering;

use penumbra_core::note::Note;
use penumbra_core::note::NoteId;

use crate::app_state::{
    apply_card_visual_model, build_note_cards, note_code, pinned_note_ids, set_status, SharedState,
    SAVE_DEBOUNCE,
};
use crate::rich_editor::RichEditor;
use crate::universe::{publish_drift, publish_visuals, start_ambient_settle, start_camera_glide};
use crate::{platform, AppWindow};
use slint::ComponentHandle;

/// Reveal the editor once its zoom has settled and its session exists.
pub(crate) fn maybe_reveal_editor(state: &SharedState, ui: &AppWindow) {
    if !state.editor_reveal.load(Ordering::Relaxed) {
        return;
    }
    let has_session = state.editor.lock().unwrap().is_some();
    if has_session {
        state.editor_reveal.store(false, Ordering::Relaxed);
        ui.set_editor_open(true);
    }
}

/// Keep the note's card on the map
pub(crate) fn live_patch_card(state: &SharedState, _ui: &AppWindow) {
    let preview = {
        let editor_guard = state.editor.lock().unwrap();
        let Some(session) = editor_guard.as_ref() else {
            return;
        };
        session.text().chars().take(80).collect::<String>()
    };
    let title = {
        let title_guard = state.editor_title.lock().unwrap();
        let Some(title) = title_guard.as_ref() else {
            return;
        };
        title.clone()
    };
    let Some(note_id) = *state.editor_note.lock().unwrap() else {
        return;
    };
    let card_id = note_code(&note_id);

    let mut cards = state.cards.lock().unwrap();
    let Some(idx) = cards.iter().position(|card| card.id == card_id) else {
        return;
    };
    let mut row = cards[idx].clone();
    if row.title.as_str() == title.as_str() && row.preview.as_str() == preview.as_str() {
        return;
    }
    row.title = title.into();
    row.preview = preview.into();
    cards[idx] = row.clone();
    drop(cards);
    apply_card_visual_model(idx, row);
}

/// Prepare the editor session
pub(crate) fn prepare_editor_session(
    state: &Rc<SharedState>,
    ui: &AppWindow,
    note_id: NoteId,
    title: String,
    body: String,
) {
    *state.editor.lock().unwrap() = Some(RichEditor::new(&body));
    *state.editor_note.lock().unwrap() = Some(note_id);
    *state.editor_title.lock().unwrap() = Some(title.clone());
    state.editor_dirty.store(false, Ordering::Relaxed);
    ui.set_note_title(title.into());
    refresh_rich_editor_surface(state, ui);
    let glide_done = state.glide.lock().unwrap().is_none();
    if glide_done {
        maybe_reveal_editor(state, ui);
    }
}

pub(crate) fn finish_editor_session(state: &Rc<SharedState>, ui: &AppWindow) {
    let pending: Option<(NoteId, String, String)> =
        if state.editor_dirty.swap(false, Ordering::Relaxed) {
            let editor = state.editor.lock().unwrap();
            let note_edit = state.editor_note.lock().unwrap();
            let title_edit = state.editor_title.lock().unwrap();
            match (editor.as_ref(), note_edit.as_ref(), title_edit.as_ref()) {
                (Some(session), Some(note_id), Some(title)) => {
                    Some((*note_id, session.to_markdown(), title.clone()))
                }
                _ => None,
            }
        } else {
            None
        };
    state.edit_pending.fetch_add(1, Ordering::Relaxed);
    *state.editor.lock().unwrap() = None;
    *state.editor_note.lock().unwrap() = None;
    *state.editor_title.lock().unwrap() = None;
    ui.set_editor_open(false);
    if let Some((note_id, body, title)) = pending {
        let state_inner = Rc::clone(state);
        let uiw = ui.as_weak();
        platform::spawn(async move {
            apply_editor_save(&state_inner, &uiw, note_id, body, title).await;
        });
    }
    restore_editor_camera(state, ui);
}

fn restore_editor_camera(state: &Rc<SharedState>, ui: &AppWindow) {
    if let Some(prev) = state.editor_prev_camera.lock().unwrap().take() {
        start_camera_glide(state, ui, prev);
    }
}

fn schedule_editor_save(state: &Rc<SharedState>, ui: slint::Weak<AppWindow>) {
    state.editor_dirty.store(true, Ordering::Relaxed);
    let epoch = state.edit_pending.fetch_add(1, Ordering::Relaxed) + 1;
    let state = Rc::clone(state);
    slint::Timer::single_shot(SAVE_DEBOUNCE, move || {
        if state.edit_pending.load(Ordering::Relaxed) != epoch {
            return;
        }
        let state_task = Rc::clone(&state);
        platform::spawn(flush_editor_changes(state_task, ui));
    });
}

pub(crate) fn refresh_rich_editor_surface(state: &SharedState, ui: &AppWindow) {
    let mut guard = state.editor.lock().unwrap();
    let Some(editor) = guard.as_mut() else {
        return;
    };
    let surface = editor.surface();
    let status = editor.status();
    let caret = editor.caret_rect();
    let focused = editor.is_focused();
    drop(guard);
    ui.set_editor_surface(surface);
    ui.set_editor_status(status.into());
    // Hide the caret when its layout position is unresolvable
    match caret {
        Some((caret_x, caret_y, caret_h)) => {
            ui.set_editor_caret_x(caret_x);
            ui.set_editor_caret_y(caret_y);
            ui.set_editor_caret_h(caret_h);
            ui.set_editor_caret_on(focused);
        }
        None => {
            ui.set_editor_caret_on(false);
        }
    }
    ui.window().request_redraw();
}

async fn flush_editor_changes(state: Rc<SharedState>, ui: slint::Weak<AppWindow>) {
    if !state.editor_dirty.swap(false, Ordering::Relaxed) {
        return;
    }
    let (note_id, body, title) = {
        let editor = state.editor.lock().unwrap();
        let note_edit = state.editor_note.lock().unwrap();
        let title_edit = state.editor_title.lock().unwrap();
        let (Some(session), Some(note_id), Some(title)) =
            (editor.as_ref(), note_edit.as_ref(), title_edit.as_ref())
        else {
            return;
        };
        (*note_id, session.to_markdown(), title.clone())
    };
    apply_editor_save(&state, &ui, note_id, body, title).await;
}

async fn apply_editor_save(
    state: &Rc<SharedState>,
    ui: &slint::Weak<AppWindow>,
    note_id: NoteId,
    body: String,
    title: String,
) {
    let note = {
        let mut universe_guard = state.universe.lock().await;
        let Some(universe) = universe_guard.as_mut() else {
            return;
        };
        let mut note = match universe.graph().get_note(&note_id) {
            Some(note) => note.clone(),
            None => {
                set_status(ui, "note disappeared");
                return;
            }
        };
        note.body = body;
        note.title = title;
        note.touch();
        if let Err(e) = universe.save_note(note.clone()).await {
            set_status(ui, &format!("could not save note: {e}"));
            return;
        }
        note
    };
    run_pipeline(state, ui, note).await;
    set_status(ui, "saved");
}

async fn run_pipeline(state: &Rc<SharedState>, ui: &slint::Weak<AppWindow>, note: Note) {
    let mut layout_guard = state.layout.lock().await;
    let Some(layout) = layout_guard.as_mut() else {
        return;
    };

    let linker = state.linker.lock().unwrap().clone();
    let mut links_changed = false;
    if let Some(linker) = linker {
        match linker.process_note(&note).await {
            Ok(created) if !created.is_empty() => {
                tracing::info!("auto-link: {} new links", created.len());
                links_changed = true;
            }
            Ok(_) => {}
            Err(e) => tracing::error!("auto-link failed: {e}"),
        }
    }

    {
        let mut universe_guard = state.universe.lock().await;
        let Some(universe) = universe_guard.as_mut() else {
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
        layout.step_neighborhood(&note.id);

        let pin_codes = state.pins.lock().unwrap().keys().copied().collect();
        let pinned_ids = pinned_note_ids(universe, &pin_codes);
        let cards = build_note_cards(layout, &universe.graph(), &pinned_ids);
        let links: Vec<(i32, i32)> = graph_links
            .iter()
            .map(|link| (note_code(&link.source), note_code(&link.target)))
            .collect();
        let settled: Vec<(i32, (f32, f32))> = layout
            .all_positions()
            .iter()
            .map(|(note_id, position)| (note_code(note_id), (position.x as f32, position.y as f32)))
            .collect();
        drop(universe_guard);
        drop(layout_guard);

        if let Some(ui) = ui.upgrade() {
            publish_visuals(state, &ui, cards, links);
            if links_changed {
                // New edges reshape the whole constellation
                start_ambient_settle(state, &ui);
            } else {
                publish_drift(state, &ui, settled);
            }
        }
    }
}

/// Apply `action` to the live editor session (if one is open) and repaint the
/// surface
fn with_session_refresh<F>(state: &SharedState, ui: &AppWindow, action: F)
where
    F: FnOnce(&mut RichEditor),
{
    if let Some(session) = state.editor.lock().unwrap().as_mut() {
        action(session);
    }
    refresh_rich_editor_surface(state, ui);
}

/// Apply `action` to the live editor session, repaint, mirror the change onto
/// the note's map card, and schedule a debounced save
fn with_session_edit<F>(state: &Rc<SharedState>, handle: &slint::Weak<AppWindow>, action: F)
where
    F: FnOnce(&mut RichEditor),
{
    let Some(ui) = handle.upgrade() else {
        return;
    };
    {
        let mut guard = state.editor.lock().unwrap();
        let Some(session) = guard.as_mut() else {
            return;
        };
        action(session);
    }
    refresh_rich_editor_surface(state, &ui);
    live_patch_card(state, &ui);
    schedule_editor_save(state, handle.clone());
}

pub(crate) fn wire_editor(ui: &AppWindow, state: &Rc<SharedState>) {
    macro_rules! on_view {
        ($setter:ident, |$session:ident $(, $arg:ident)*| $body:expr) => {{
            let state = Rc::clone(state);
            let handle = ui.as_weak();
            ui.$setter(move |$($arg),*| {
                let Some(ui) = handle.upgrade() else {
                    return;
                };
                with_session_refresh(&state, &ui, |$session| $body);
            });
        }};
    }

    macro_rules! on_edit {
        ($setter:ident, |$session:ident $(, $arg:ident)*| $body:expr) => {{
            let state = Rc::clone(state);
            let handle = ui.as_weak();
            ui.$setter(move |$($arg),*| {
                with_session_edit(&state, &handle, |$session| $body);
            });
        }};
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_close_editor(move || {
            let Some(ui) = handle.upgrade() else {
                return;
            };
            finish_editor_session(&state, &ui);
        });
    }

    {
        let state = Rc::clone(state);
        let handle = ui.as_weak();
        ui.on_editor_title_changed(move |text| {
            let Some(ui) = handle.upgrade() else {
                return;
            };
            *state.editor_title.lock().unwrap() = Some(text.into());
            live_patch_card(&state, &ui);
            schedule_editor_save(&state, handle.clone());
        });
    }

    on_edit!(on_editor_undo, |session| session.undo());
    on_edit!(on_editor_redo, |session| session.redo());
    on_edit!(
        on_editor_input_key,
        |session, text, shift, control, meta, alt| session
            .input_key(&text, shift, control, meta, alt)
    );
    on_edit!(on_editor_cut_selection, |session| session.cut_selection());
    on_edit!(on_editor_paste_clipboard, |session| session
        .paste_clipboard());

    on_view!(on_editor_focus_changed, |session, focused| session
        .set_focused(focused));
    on_view!(on_editor_surface_resized, |session, width, height| session
        .set_size(
            width.max(0.0).round() as u32,
            height.max(0.0).round() as u32
        ));
    on_view!(on_editor_pointer_down, |session, x, y, extend| session
        .pointer_down(x, y, extend));
    on_view!(on_editor_pointer_drag, |session, x, y| session
        .pointer_drag(x, y));
    on_view!(on_editor_pointer_double_click, |session, x, y| session
        .pointer_double_click(x, y));
    on_view!(on_editor_pointer_triple_click, |session, x, y| session
        .pointer_triple_click(x, y));
    on_view!(on_editor_pointer_scroll, |session, pixels| session
        .pointer_scroll(pixels));
    on_view!(on_editor_toggle_bold, |session| session.toggle_bold());
    on_view!(on_editor_toggle_italic, |session| session.toggle_italic());
    on_view!(on_editor_copy_selection, |session| session.copy_selection());
    on_view!(on_editor_select_all, |session| session.select_all());
}
