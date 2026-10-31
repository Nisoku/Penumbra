pub mod display;
pub mod map;
mod platform;
pub mod rich_editor;

mod app_state;
mod editor_session;
mod universe;

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app_state::{
    build_note_cards, note_code, pinned_note_ids, set_status, SharedState, OPEN_ZOOM_DURATION,
    PERSIST_INTERVAL, WELCOME_BODY, WELCOME_TITLE,
};
use crate::editor_session::{prepare_editor_session, wire_editor};
use crate::map::MapCamera;
use crate::universe::{
    cancel_camera_glide, center_camera_on_cards, publish_visuals, rebuild_paths, start_glide,
    viewport_size, wire_map,
};
use penumbra_app::Universe;
use penumbra_auto_link::AutoLinker;
use penumbra_core::error::Result as PenumbraResult;
use penumbra_core::note::Note;
use penumbra_core::note::NoteId;
use penumbra_core::EmbeddingProvider;
use penumbra_embed::SimpleEmbedder;
use penumbra_index::VectorIndex;
use penumbra_layout::LayoutEngine;
use penumbra_storage::Storage;
use slint::ComponentHandle;

slint::include_modules!();

pub fn run() {
    start_app().expect("penumbra failed to start");
}

fn start_app() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let state = Rc::new(SharedState {
        universe: futures::lock::Mutex::new(None),
        layout: futures::lock::Mutex::new(None),
        cards: Mutex::new(Vec::new()),
        links: Mutex::new(Vec::new()),
        pins: Mutex::new(std::collections::HashMap::new()),
        selected: std::sync::atomic::AtomicI32::new(-1),
        dirty: std::sync::atomic::AtomicBool::new(false),
        drift: Mutex::new(None),
        editor: Mutex::new(None),
        editor_note: Mutex::new(None),
        editor_title: Mutex::new(None),
        editor_dirty: std::sync::atomic::AtomicBool::new(false),
        edit_pending: std::sync::atomic::AtomicI32::new(0),
        glide: Mutex::new(None),
        editor_prev_camera: Mutex::new(None),
        editor_reveal: std::sync::atomic::AtomicBool::new(false),
        embedder: Mutex::new(None),
        linker: Mutex::new(None),
    });

    wire_new_note(&ui, &state);
    wire_editor(&ui, &state);
    wire_open_note(&ui, &state);
    wire_map(&ui, &state);

    // `_persist_timer` lives to the end of this scope, keeping the timer armed
    // for the whole loop; dropping it after the loop exits unregisters it.
    let _persist_timer = slint::Timer::default();
    let timer_state = Rc::clone(&state);
    _persist_timer.start(slint::TimerMode::Repeated, PERSIST_INTERVAL, move || {
        let state = Rc::clone(&timer_state);
        platform::spawn(async move {
            persist_snapshot(&state).await;
        });
    });

    // The first event-loop iteration arms boot so `platform::spawn` always
    // runs with the loop already spinning. `single_shot` self-registers.
    let boot_state = Rc::clone(&state);
    let boot_ui = ui.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        let state = Rc::clone(&boot_state);
        let ui = boot_ui.clone();
        platform::spawn(async move {
            run_boot(state, ui).await;
        });
    });

    ui.run()?;

    let state_ref = Rc::clone(&state);
    platform::block_on(persist_snapshot(&state_ref));
    Ok(())
}

async fn run_boot(state: Rc<SharedState>, boot_ui: slint::Weak<AppWindow>) {
    set_status(&boot_ui, "loading embeddings...");

    #[cfg(feature = "candle-load")]
    {
        match penumbra_embed::candle::CandleEmbedder::load_cached().await {
            Ok(_emb) => set_status(&boot_ui, "embeddings ready"),
            Err(e) => set_status(&boot_ui, &format!("embedding load failed: {e}")),
        }
    }

    match boot_universe().await {
        Ok(mut universe) => {
            if universe.note_count() == 0 {
                if let Err(err) = universe
                    .create_note(WELCOME_TITLE.to_string(), WELCOME_BODY.to_string())
                    .await
                {
                    report_boot_failure(boot_ui.clone(), &err.to_string());
                    return;
                }
            }

            let count = universe.note_count();

            let mut layout = LayoutEngine::with_defaults().await;

            let saved_positions = universe
                .storage()
                .load_positions()
                .await
                .unwrap_or_default();
            let saved_pins = match universe.storage().load_pins().await {
                Ok(Some(set)) => set,
                _ => HashSet::new(),
            };

            for note in universe.graph().all_notes() {
                layout.add_node(note.id, saved_pins.contains(&note.id));
                if let Some(ref positions) = saved_positions {
                    if let Some(&pos) = positions.get(&note.id) {
                        layout.set_position(&note.id, pos);
                    }
                }
            }

            let graph_links: Vec<_> = universe.graph().all_links().into_iter().cloned().collect();
            layout.update_links(graph_links.clone());
            layout.run();

            let graph_handle = universe.graph_handle();
            let events = universe.events();
            let embedder: Arc<dyn EmbeddingProvider> = Arc::new(SimpleEmbedder::new_384());
            let index: Arc<Mutex<dyn VectorIndex>> = Arc::new(Mutex::new(
                penumbra_index::RuvectorIndex::new(384).expect("index for the 384-dim embedder"),
            ));
            {
                let notes: Vec<Note> = universe.graph().all_notes().cloned().collect();
                for note in notes {
                    let embedding = match embedder.embed_note(&note).await {
                        Ok(embedding) => embedding,
                        Err(e) => {
                            tracing::warn!("embedding failed for {}: {e}", note.id);
                            continue;
                        }
                    };
                    let mut idx = index
                        .lock()
                        .expect("the boot pipeline index lock is uncontended");
                    if let Err(e) = idx.insert(note.id, &embedding) {
                        tracing::warn!("index insert failed for {}: {e}", note.id);
                    }
                }
            }
            let linker = AutoLinker::with_defaults(
                Arc::clone(&embedder),
                Arc::clone(&index),
                graph_handle,
                events,
            );
            *state.embedder.lock().unwrap() = Some(embedder);
            *state.linker.lock().unwrap() = Some(Arc::new(linker));

            {
                let positions = layout.all_positions();
                let mut pins = state.pins.lock().unwrap();
                for note_id in &saved_pins {
                    if let Some(&pos) = positions.get(note_id) {
                        pins.insert(note_code(note_id), (pos.x as f32, pos.y as f32));
                    }
                }
            }

            let cards = build_note_cards(&layout, &universe.graph(), &saved_pins);
            let links: Vec<(i32, i32)> = graph_links
                .iter()
                .map(|link| (note_code(&link.source), note_code(&link.target)))
                .collect();
            *state.layout.lock().await = Some(layout);
            *state.universe.lock().await = Some(universe);

            let boot_state = Rc::clone(&state);
            if let Some(ui) = boot_ui.upgrade() {
                ui.set_ready(true);
                ui.set_status(format!("universe ready - {} notes", count).into());
                publish_visuals(&boot_state, &ui, cards, links);
                center_camera_on_cards(&boot_state, &ui);
                rebuild_paths(&boot_state, &ui);
            }
        }
        Err(err) => report_boot_failure(boot_ui, &err.to_string()),
    }
}

async fn boot_universe() -> PenumbraResult<Universe> {
    let root = platform::vault_root().await?;
    let storage = Storage::with_dir(root).await;
    Universe::open(storage).await
}

async fn persist_snapshot(state: &SharedState) {
    let mut layout_guard = state.layout.lock().await;
    let universe_guard = state.universe.lock().await;
    if let (Some(layout), Some(universe)) = (layout_guard.as_mut(), universe_guard.as_ref()) {
        let positions = layout.all_positions();
        if let Err(e) = universe.storage().save_positions(&positions).await {
            tracing::error!("failed to save positions: {e}");
        }
        let pin_codes: HashSet<i32> = state.pins.lock().unwrap().keys().copied().collect();
        let pinned_ids = pinned_note_ids(universe, &pin_codes);
        if let Err(e) = universe.storage().save_pins(&pinned_ids).await {
            tracing::error!("failed to save pins: {e}");
        }
    }
}

async fn load_note_for_editor(
    state: &Rc<SharedState>,
    card_id: i32,
) -> Option<(NoteId, String, String)> {
    let guard = state.universe.lock().await;
    let universe = guard.as_ref()?;
    let (note_id, title, body) = {
        let note = universe
            .graph()
            .all_notes()
            .find(|note| note_code(&note.id) == card_id)
            .cloned()?;
        (note.id, note.title.clone(), note.body.clone())
    };
    Some((note_id, title, body))
}

fn wire_open_note(ui: &AppWindow, state: &Rc<SharedState>) {
    let state = Rc::clone(state);
    let handle = ui.as_weak();
    ui.on_open_note(move |card_id| {
        let state_outer = Rc::clone(&state);
        let state_task = Rc::clone(&state_outer);
        start_open_zoom(&state_outer, &handle, card_id);
        let ui = handle.clone();
        platform::spawn(async move {
            let Some((note_id, title, body)) = load_note_for_editor(&state_task, card_id).await
            else {
                set_status(&ui, "note not found");
                cancel_open_zoom(&state_task);
                return;
            };
            if let Some(ui) = ui.upgrade() {
                prepare_editor_session(&state_task, &ui, note_id, title, body);
            }
        });
    });
}

/// Begin the zoom-into-card
fn start_open_zoom(state: &Rc<SharedState>, ui: &slint::Weak<AppWindow>, card_id: i32) {
    let Some(ui) = ui.upgrade() else {
        return;
    };
    let cards = state.cards.lock().unwrap();
    let Some(card) = cards.iter().find(|card| card.id == card_id) else {
        return;
    };
    let (view_w, view_h) = viewport_size(&ui);
    let target = map::zoom_card(card.x, card.y, view_w, view_h);
    drop(cards);
    let from = MapCamera {
        x: ui.get_camera_x(),
        y: ui.get_camera_y(),
        zoom: ui.get_zoom(),
    };
    *state.editor_prev_camera.lock().unwrap() = Some(from);
    state.editor_reveal.store(true, Ordering::Relaxed);
    start_glide(state, &ui, target, OPEN_ZOOM_DURATION);
}

/// Abort a zoom-in that will not be followed by an editor reveal (for
/// example when the note failed to load).
fn cancel_open_zoom(state: &SharedState) {
    state.editor_reveal.store(false, Ordering::Relaxed);
    cancel_camera_glide(state);
}

fn wire_new_note(ui: &AppWindow, state: &Rc<SharedState>) {
    let state = Rc::clone(state);
    let handle = ui.as_weak();
    ui.on_new_note(move || {
        let state = Rc::clone(&state);
        let ui = handle.clone();
        platform::spawn(async move {
            new_note_task(state, ui).await;
        });
    });
}

async fn new_note_task(state: Rc<SharedState>, ui: slint::Weak<AppWindow>) {
    let mut layout_guard = state.layout.lock().await;
    let mut guard = state.universe.lock().await;
    let Some(universe) = guard.as_mut() else {
        return;
    };
    match universe
        .create_note("Untitled".to_string(), String::new())
        .await
    {
        Ok(_id) => {
            let count = universe.note_count();
            if let Some(ref mut layout) = *layout_guard {
                layout.add_node(_id, false);
                let graph_links: Vec<_> =
                    universe.graph().all_links().into_iter().cloned().collect();
                layout.update_links(graph_links.clone());
                layout.step();
                let pin_codes = state.pins.lock().unwrap().keys().copied().collect();
                let pinned_ids = pinned_note_ids(universe, &pin_codes);
                let cards = build_note_cards(layout, &universe.graph(), &pinned_ids);
                let links: Vec<(i32, i32)> = graph_links
                    .iter()
                    .map(|link| (note_code(&link.source), note_code(&link.target)))
                    .collect();
                drop(guard);
                drop(layout_guard);
                if let Some(ui) = ui.upgrade() {
                    ui.set_status(format!("universe ready - {} notes", count).into());
                    publish_visuals(&state, &ui, cards, links);
                }
            }
        }
        Err(err) => {
            let message = format!("could not save note: {}", err);
            if let Some(ui) = ui.upgrade() {
                ui.set_status(message.into());
            }
        }
    }
}

fn report_boot_failure(ui: slint::Weak<AppWindow>, message: &str) {
    let text = format!("could not open universe: {}", message);
    if let Some(ui) = ui.upgrade() {
        ui.set_status(text.into());
    }
}
