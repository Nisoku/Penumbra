mod vault;

use std::sync::Arc;

use penumbra_app::Universe;
use penumbra_core::error::Result as PenumbraResult;
use penumbra_storage::Storage;
use slint::ComponentHandle;
use tokio::sync::Mutex as AsyncMutex;

slint::include_modules!();

const WELCOME_TITLE: &str = "Welcome to Penumbra";
const WELCOME_BODY: &str = "This is your first note.\n\nEverything you write lands on the map over time. Drag notes around, link them, and watch constellations form.";

struct SharedState {
    universe: AsyncMutex<Option<Universe>>,
    #[cfg(not(target_family = "wasm"))]
    handle: tokio::runtime::Handle,
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
        handle: runtime.handle().clone(),
    });

    wire_new_note(&ui, &state);
    wire_editor(&ui);
    wire_open_note(&ui);

    let boot_state = Arc::clone(&state);
    let boot_ui = ui.as_weak();
    runtime.spawn(async move {
        run_boot(boot_state, boot_ui).await;
    });

    ui.run()?;
    drop(runtime);
    Ok(())
}

#[cfg(target_family = "wasm")]
fn start_app() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let state = Arc::new(SharedState {
        universe: AsyncMutex::new(None),
    });

    wire_new_note(&ui, &state);
    wire_editor(&ui);
    wire_open_note(&ui);

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
            let cards = build_note_cards(&universe);
            *state.universe.lock().await = Some(universe);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = boot_ui.upgrade() {
                    ui.set_ready(true);
                    ui.set_status(format!("universe ready - {} notes", count).into());
                    ui.set_note_cards(slint::ModelRc::new(slint::VecModel::from(cards)));
                }
            });
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

fn build_note_cards(universe: &Universe) -> Vec<NoteCardVM> {
    universe
        .graph()
        .all_notes()
        .enumerate()
        .map(|(i, note)| {
            let col = (i % 5) as f32;
            let row = (i / 5) as f32;
            NoteCardVM {
                id: note.id.as_uuid().as_u128() as i32,
                title: note.title.as_str().into(),
                preview: note
                    .body
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .as_str()
                    .into(),
                tags: note.tags.join(", ").as_str().into(),
                x: 24.0 + col * 200.0,
                y: 24.0 + row * 140.0,
            }
        })
        .collect()
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
        Ok(_) => {
            let count = universe.note_count();
            let cards = build_note_cards(universe);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_status(format!("universe ready - {} notes", count).into());
                    ui.set_note_cards(slint::ModelRc::new(slint::VecModel::from(cards)));
                }
            });
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

fn report_boot_failure(ui: slint::Weak<AppWindow>, message: &str) {
    let text = format!("could not open universe: {}", message);
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.set_status(text.into());
        }
    });
}
