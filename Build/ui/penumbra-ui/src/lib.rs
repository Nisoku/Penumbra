pub mod map;
mod vault;

use std::collections::HashMap;
#[cfg(not(target_family = "wasm"))]
use std::collections::HashSet;
use std::rc::Rc;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use penumbra_app::Universe;
use penumbra_core::error::Result as PenumbraResult;
use penumbra_core::note::NoteId;
#[cfg(not(target_family = "wasm"))]
use penumbra_core::position::Position;
#[cfg(not(target_family = "wasm"))]
use penumbra_layout::LayoutEngine;
use penumbra_storage::Storage;
use slint::ComponentHandle;
use slint::Model;
use tokio::sync::Mutex as AsyncMutex;

slint::include_modules!();

thread_local! {
    static UI_CARDS_MODEL: std::cell::RefCell<Option<Rc<slint::VecModel<NoteCardVM>>>> =
        const { std::cell::RefCell::new(None) };
}

const WELCOME_TITLE: &str = "Welcome to Penumbra";
const WELCOME_BODY: &str = "This is your first note.\n\nEverything you write lands on the map over time. Drag notes around, link them, and watch constellations form.";

const TOP_BAR_HEIGHT: f32 = 52.0;

#[cfg(not(target_family = "wasm"))]
use std::time::{Duration, Instant};

#[cfg(not(target_family = "wasm"))]
const DRIFT_DURATION: Duration = Duration::from_millis(600);
#[cfg(not(target_family = "wasm"))]
const DRIFT_TICK: Duration = Duration::from_millis(16);

#[cfg(not(target_family = "wasm"))]
struct SharedState {
    universe: AsyncMutex<Option<Universe>>,
    layout: AsyncMutex<Option<LayoutEngine>>,
    handle: tokio::runtime::Handle,
    cards: Mutex<Vec<NoteCardVM>>,
    links: Mutex<Vec<(i32, i32)>>,
    pins: Mutex<HashMap<i32, (f32, f32)>>,
    selected: AtomicI32,
    dirty: AtomicBool,
    drift: Mutex<Option<Drift>>,
}

#[cfg(target_family = "wasm")]
struct SharedState {
    universe: AsyncMutex<Option<Universe>>,
    cards: Mutex<Vec<NoteCardVM>>,
    links: Mutex<Vec<(i32, i32)>>,
    pins: Mutex<HashMap<i32, (f32, f32)>>,
    selected: AtomicI32,
}

#[cfg(not(target_family = "wasm"))]
struct DriftTarget {
    card_id: i32,
    from: (f32, f32),
    to: (f32, f32),
}

#[cfg(not(target_family = "wasm"))]
struct Drift {
    items: Vec<DriftTarget>,
    started: Instant,
    duration: Duration,
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn run() {
    start_app().expect("penumbra failed to start");
}

#[cfg(not(target_family = "wasm"))]
fn start_app() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let state = Arc::new(SharedState {
        universe: AsyncMutex::new(None),
        layout: AsyncMutex::new(None),
        handle: runtime.handle().clone(),
        cards: Mutex::new(Vec::new()),
        links: Mutex::new(Vec::new()),
        pins: Mutex::new(HashMap::new()),
        selected: AtomicI32::new(-1),
        dirty: AtomicBool::new(false),
        drift: Mutex::new(None),
    });

    wire_new_note(&ui, &state);
    wire_editor(&ui);
    wire_open_note(&ui);
    wire_map(&ui, &state);

    {
        let state = Arc::clone(&state);
        runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                if !state.dirty.swap(false, Ordering::Relaxed) {
                    continue;
                }
                persist_snapshot(&state).await;
            }
        });
    }

    let boot_state = Arc::clone(&state);
    let boot_ui = ui.as_weak();
    runtime.spawn(async move {
        run_boot(boot_state, boot_ui).await;
    });

    ui.run()?;

    {
        let state_ref = Arc::clone(&state);
        runtime.block_on(async move {
            persist_snapshot(&state_ref).await;
        });
    }

    drop(runtime);
    Ok(())
}

#[cfg(target_family = "wasm")]
fn start_app() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let state = Arc::new(SharedState {
        universe: AsyncMutex::new(None),
        cards: Mutex::new(Vec::new()),
        links: Mutex::new(Vec::new()),
        pins: Mutex::new(HashMap::new()),
        selected: AtomicI32::new(-1),
    });

    wire_new_note(&ui, &state);
    wire_editor(&ui);
    wire_open_note(&ui);
    wire_map(&ui, &state);

    let boot_state = Arc::clone(&state);
    let boot_ui = ui.as_weak();
    wasm_bindgen_futures::spawn_local(async move {
        run_boot(boot_state, boot_ui).await;
    });

    ui.run()?;
    Ok(())
}

async fn run_boot(state: Arc<SharedState>, boot_ui: slint::Weak<AppWindow>) {
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

            #[cfg(not(target_family = "wasm"))]
            {
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

                let graph_links: Vec<_> =
                    universe.graph().all_links().into_iter().cloned().collect();
                layout.update_links(graph_links.clone());
                layout.run();

                {
                    let positions = layout.all_positions();
                    let mut pins = state.pins.lock().unwrap();
                    for note_id in &saved_pins {
                        if let Some(&pos) = positions.get(note_id) {
                            pins.insert(note_code(note_id), (pos.x as f32, pos.y as f32));
                        }
                    }
                }

                let cards = build_note_cards(&layout, universe.graph(), &saved_pins);
                let links: Vec<(i32, i32)> = graph_links
                    .iter()
                    .map(|link| (note_code(&link.source), note_code(&link.target)))
                    .collect();
                *state.layout.lock().await = Some(layout);
                *state.universe.lock().await = Some(universe);

                let boot_state = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = boot_ui.upgrade() {
                        ui.set_ready(true);
                        ui.set_status(format!("universe ready - {} notes", count).into());
                        publish_visuals(&boot_state, &ui, cards, links);
                        center_camera_on_cards(&boot_state, &ui);
                        rebuild_paths(&boot_state, &ui);
                    }
                });
            }

            #[cfg(target_family = "wasm")]
            {
                let cards = build_note_cards_fallback(&universe);
                let graph_links: Vec<_> =
                    universe.graph().all_links().into_iter().cloned().collect();
                let links: Vec<(i32, i32)> = graph_links
                    .iter()
                    .map(|link| (note_code(&link.source), note_code(&link.target)))
                    .collect();
                *state.universe.lock().await = Some(universe);

                let boot_state = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = boot_ui.upgrade() {
                        ui.set_ready(true);
                        ui.set_status(format!("universe ready - {} notes", count).into());
                        publish_visuals(&boot_state, &ui, cards, links);
                        center_camera_on_cards(&boot_state, &ui);
                        rebuild_paths(&boot_state, &ui);
                    }
                });
            }
        }
        Err(err) => report_boot_failure(boot_ui, &err.to_string()),
    }
}

fn set_status(ui: &slint::Weak<AppWindow>, msg: &str) {
    let ui = ui.clone();
    let msg = msg.to_owned();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.set_status(msg.into());
        }
    });
}

async fn boot_universe() -> PenumbraResult<Universe> {
    let root = vault::resolve_root().await?;
    let storage = Storage::with_dir(root).await;
    Universe::open(storage).await
}

#[cfg(not(target_family = "wasm"))]
fn build_note_cards(
    layout: &LayoutEngine,
    graph: &penumbra_graph::GraphStore,
    pinned: &HashSet<NoteId>,
) -> Vec<NoteCardVM> {
    let positions = layout.all_positions();
    graph
        .all_notes()
        .map(|note| {
            let pos = positions
                .get(&note.id)
                .copied()
                .unwrap_or(Position::new(0.0, 0.0));
            NoteCardVM {
                id: note_code(&note.id),
                title: note.title.as_str().into(),
                preview: note
                    .body
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .as_str()
                    .into(),
                tags: note.tags.join(", ").as_str().into(),
                pinned: pinned.contains(&note.id),
                x: pos.x as f32,
                y: pos.y as f32,
            }
        })
        .collect()
}

#[cfg(target_family = "wasm")]
fn build_note_cards_fallback(universe: &Universe) -> Vec<NoteCardVM> {
    universe
        .graph()
        .all_notes()
        .enumerate()
        .map(|(i, note)| {
            let col = (i % 5) as f32;
            let row = (i / 5) as f32;
            NoteCardVM {
                id: note_code(&note.id),
                title: note.title.as_str().into(),
                preview: note
                    .body
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .as_str()
                    .into(),
                tags: note.tags.join(", ").as_str().into(),
                pinned: false,
                x: 24.0 + col * 200.0,
                y: 24.0 + row * 140.0,
            }
        })
        .collect()
}

fn note_code(id: &NoteId) -> i32 {
    id.as_uuid().as_u128() as i32
}

#[cfg(not(target_family = "wasm"))]
fn pinned_note_ids(universe: &Universe, pin_codes: &HashSet<i32>) -> HashSet<NoteId> {
    universe
        .graph()
        .all_notes()
        .map(|note| note.id)
        .filter(|id| pin_codes.contains(&note_code(id)))
        .collect()
}

#[cfg(not(target_family = "wasm"))]
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

fn wire_open_note(ui: &AppWindow) {
    let handle = ui.as_weak();
    ui.on_open_note(move |card_id| {
        let ui = handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui.upgrade() else {
                return;
            };
            let demo = vec![
                BlockVM {
                    id: 0,
                    text: "Welcome to Penumbra".into(),
                    is_active: card_id == 0,
                    kind: "heading".into(),
                },
                BlockVM {
                    id: 1,
                    text: "This is your first note.".into(),
                    is_active: card_id == 1,
                    kind: "paragraph".into(),
                },
            ];
            let model = slint::VecModel::from(demo);
            ui.set_editor_blocks(slint::ModelRc::new(model));
            ui.set_editor_open(true);
        });
    });
}

#[cfg(not(target_family = "wasm"))]
fn wire_new_note(ui: &AppWindow, state: &Arc<SharedState>) {
    let state = Arc::clone(state);
    let handle = ui.as_weak();
    ui.on_new_note(move || {
        let state = Arc::clone(&state);
        let ui = handle.clone();
        state.handle.spawn({
            let state = Arc::clone(&state);
            async move {
                new_note_task(state, ui).await;
            }
        });
    });
}

#[cfg(target_family = "wasm")]
fn wire_new_note(ui: &AppWindow, state: &Arc<SharedState>) {
    let state = Arc::clone(state);
    let handle = ui.as_weak();
    ui.on_new_note(move || {
        let state = Arc::clone(&state);
        let ui = handle.clone();
        wasm_bindgen_futures::spawn_local(async move {
            new_note_task(state, ui).await;
        });
    });
}

async fn new_note_task(state: Arc<SharedState>, ui: slint::Weak<AppWindow>) {
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

            #[cfg(not(target_family = "wasm"))]
            {
                let mut layout_guard = state.layout.lock().await;
                if let Some(ref mut layout) = *layout_guard {
                    layout.add_node(_id, false);
                    let graph_links: Vec<_> =
                        universe.graph().all_links().into_iter().cloned().collect();
                    layout.update_links(graph_links.clone());
                    layout.step();
                    let pin_codes = state.pins.lock().unwrap().keys().copied().collect();
                    let pinned_ids = pinned_note_ids(universe, &pin_codes);
                    let cards = build_note_cards(layout, universe.graph(), &pinned_ids);
                    let links: Vec<(i32, i32)> = graph_links
                        .iter()
                        .map(|link| (note_code(&link.source), note_code(&link.target)))
                        .collect();
                    drop(guard);
                    drop(layout_guard);
                    let state = Arc::clone(&state);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui.upgrade() {
                            ui.set_status(format!("universe ready - {} notes", count).into());
                            publish_visuals(&state, &ui, cards, links);
                        }
                    });
                }
            }

            #[cfg(target_family = "wasm")]
            {
                let cards = build_note_cards_fallback(universe);
                let graph_links: Vec<_> =
                    universe.graph().all_links().into_iter().cloned().collect();
                let links: Vec<(i32, i32)> = graph_links
                    .iter()
                    .map(|link| (note_code(&link.source), note_code(&link.target)))
                    .collect();
                drop(guard);
                let state = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui.upgrade() {
                        ui.set_status(format!("universe ready - {} notes", count).into());
                        publish_visuals(&state, &ui, cards, links);
                    }
                });
            }
        }
        Err(err) => {
            let message = format!("could not save note: {}", err);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_status(message.into());
                }
            });
        }
    }
}

fn wire_editor(ui: &AppWindow) {
    let handle_a = ui.as_weak();
    ui.on_open_editor(move |block_id| {
        let ui = handle_a.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui.upgrade() else {
                return;
            };

            let demo = vec![
                BlockVM { id: 0, text: "Welcome to Penumbra".into(), is_active: block_id == 0, kind: "heading".into() },
                BlockVM { id: 1, text: "This is your first note.\n\nEverything you write lands on the map over time. Drag notes around, link them, and watch constellations form.".into(), is_active: block_id == 1, kind: "paragraph".into() },
            ];

            let model = slint::VecModel::from(demo);
            ui.set_editor_blocks(slint::ModelRc::new(model));
            ui.set_editor_open(true);
        });
    });

    let handle_b = ui.as_weak();
    ui.on_close_editor(move || {
        let ui = handle_b.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui.upgrade() else {
                return;
            };
            ui.set_editor_open(false);
            ui.set_editor_blocks(slint::ModelRc::default());
        });
    });

    ui.on_editor_text_changed(|block_id, text| {
        tracing::trace!(block_id, %text, "editor text changed");
    });
}

fn wire_map(ui: &AppWindow, state: &Arc<SharedState>) {
    {
        let state = Arc::clone(state);
        let handle = ui.as_weak();
        ui.on_camera_changed(move |_, _, _| {
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
        });
    }

    {
        let state = Arc::clone(state);
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
        let state = Arc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_moved(move |card_id, x, y| {
            #[cfg(not(target_family = "wasm"))]
            state.dirty.store(true, Ordering::Relaxed);
            apply_card_visual(&state, card_id, x, y);
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
        });
    }

    #[cfg(not(target_family = "wasm"))]
    {
        let state = Arc::clone(state);
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
            let state = Arc::clone(&state);
            let handle = handle.clone();
            let spawn_handle = state.handle.clone();
            spawn_handle.spawn(async move {
                if let Some(settled) = settle_note(state.clone(), card_id, (x, y)).await {
                    let handle = handle.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle.upgrade() {
                            publish_drift(&state, &ui, settled);
                        }
                    });
                }
            });
        });
    }

    #[cfg(target_family = "wasm")]
    ui.on_note_released(|_card_id, _x, _y| {});

    {
        let state = Arc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_pinned(move |card_id, x, y| {
            #[cfg(not(target_family = "wasm"))]
            state.dirty.store(true, Ordering::Relaxed);
            set_card_pinned(&state, card_id, true, (x, y));
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
            #[cfg(not(target_family = "wasm"))]
            {
                let state = Arc::clone(&state);
                let handle = handle.clone();
                let spawn_handle = state.handle.clone();
                spawn_handle.spawn(async move {
                    if let Some(settled) = pin_note(state.clone(), card_id, (x, y)).await {
                        let handle = handle.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle.upgrade() {
                                publish_drift(&state, &ui, settled);
                            }
                        });
                    }
                });
            }
        });
    }

    {
        let state = Arc::clone(state);
        let handle = ui.as_weak();
        ui.on_note_unpinned(move |card_id| {
            #[cfg(not(target_family = "wasm"))]
            state.dirty.store(true, Ordering::Relaxed);
            set_card_pinned(&state, card_id, false, (0.0, 0.0));
            let Some(ui) = handle.upgrade() else {
                return;
            };
            rebuild_paths(&state, &ui);
            #[cfg(not(target_family = "wasm"))]
            {
                let state = Arc::clone(&state);
                let handle = handle.clone();
                let spawn_handle = state.handle.clone();
                spawn_handle.spawn(async move {
                    if let Some(settled) = unpin_note(state.clone(), card_id).await {
                        let handle = handle.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle.upgrade() {
                                publish_drift(&state, &ui, settled);
                            }
                        });
                    }
                });
            }
        });
    }
}

fn publish_visuals(
    state: &SharedState,
    ui: &AppWindow,
    cards: Vec<NoteCardVM>,
    links: Vec<(i32, i32)>,
) {
    let model = Rc::new(slint::VecModel::from(cards.clone()));
    *state.cards.lock().unwrap() = cards;
    *state.links.lock().unwrap() = links;
    ui.set_note_cards(slint::ModelRc::new(Rc::clone(&model)));
    UI_CARDS_MODEL.with(|cell| *cell.borrow_mut() = Some(Rc::clone(&model)));
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
    UI_CARDS_MODEL.with(|cell| {
        if let Some(model) = cell.borrow().as_ref() {
            model.set_row_data(idx, row);
        }
    });
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
    UI_CARDS_MODEL.with(|cell| {
        if let Some(model) = cell.borrow().as_ref() {
            model.set_row_data(idx, row);
        }
    });
}

fn center_camera_on_cards(state: &SharedState, ui: &AppWindow) {
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

    let window = ui.window();
    let size = window.size();
    let scale = window.scale_factor();
    let view_w = size.width as f32 / scale;
    let view_h = (size.height as f32 / scale - TOP_BAR_HEIGHT).max(0.0);

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    ui.set_camera_x(center_x - view_w * 0.5);
    ui.set_camera_y(center_y - view_h * 0.5);
}

fn rebuild_paths(state: &SharedState, ui: &AppWindow) {
    let cam = map::MapCamera {
        x: ui.get_camera_x(),
        y: ui.get_camera_y(),
        zoom: ui.get_zoom(),
    };
    let window = ui.window();
    let size = window.size();
    let scale = window.scale_factor();
    let view_w = size.width as f32 / scale;
    let view_h = (size.height as f32 / scale - TOP_BAR_HEIGHT).max(0.0);

    let cards = state.cards.lock().unwrap().clone();
    let links = state.links.lock().unwrap().clone();
    let selected = state.selected.load(Ordering::Relaxed);
    let edges = map::build_edges(&cards, &links, cam, view_w, view_h, selected);
    let grid = map::build_grid(cam, view_w, view_h, map::GRID_SPACING);
    ui.set_grid_path(grid.into());
    ui.set_edge_path(edges.base.into());
    ui.set_selected_edge_path(edges.selected.into());
}

#[cfg(not(target_family = "wasm"))]
async fn settle_note(
    state: Arc<SharedState>,
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
    Some(
        layout
            .all_positions()
            .iter()
            .map(|(note_id, position)| (note_code(note_id), (position.x as f32, position.y as f32)))
            .collect(),
    )
}

#[cfg(not(target_family = "wasm"))]
async fn pin_note(
    state: Arc<SharedState>,
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
    Some(
        layout
            .all_positions()
            .iter()
            .map(|(note_id, position)| (note_code(note_id), (position.x as f32, position.y as f32)))
            .collect(),
    )
}

#[cfg(not(target_family = "wasm"))]
async fn unpin_note(state: Arc<SharedState>, card_id: i32) -> Option<Vec<(i32, (f32, f32))>> {
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
    Some(
        layout
            .all_positions()
            .iter()
            .map(|(note_id, position)| (note_code(note_id), (position.x as f32, position.y as f32)))
            .collect(),
    )
}

#[cfg(not(target_family = "wasm"))]
fn publish_drift(state: &Arc<SharedState>, ui: &AppWindow, settled: Vec<(i32, (f32, f32))>) {
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

#[cfg(not(target_family = "wasm"))]
fn arm_drift_tick(state: &Arc<SharedState>, ui: &AppWindow) {
    let state = Arc::clone(state);
    let handle = ui.as_weak();
    slint::Timer::single_shot(DRIFT_TICK, move || {
        let Some(ui) = handle.upgrade() else {
            return;
        };
        drift_step(&state, &ui);
    });
}

#[cfg(not(target_family = "wasm"))]
fn drift_step(state: &Arc<SharedState>, ui: &AppWindow) {
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

#[cfg(not(target_family = "wasm"))]
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn report_boot_failure(ui: slint::Weak<AppWindow>, message: &str) {
    let text = format!("could not open universe: {}", message);
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.set_status(text.into());
        }
    });
}
