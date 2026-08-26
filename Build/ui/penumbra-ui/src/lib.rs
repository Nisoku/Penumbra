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
    });

    wire_new_note(&ui, &state);

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

    let boot_state = Arc::clone(&state);
    let boot_ui = ui.as_weak();
    wasm_bindgen_futures::spawn_local(async move {
        run_boot(boot_state, boot_ui).await;
    });

    ui.run()?;
    Ok(())
}

async fn run_boot(state: Arc<SharedState>, boot_ui: slint::Weak<AppWindow>) {
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
            *state.universe.lock().await = Some(universe);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = boot_ui.upgrade() {
                    ui.set_ready(true);
                    ui.set_status(format!("universe ready - {} notes", count).into());
                }
            });
        }
        Err(err) => report_boot_failure(boot_ui, &err.to_string()),
    }
}

async fn boot_universe() -> PenumbraResult<Universe> {
    let root = vault::resolve_root().await?;
    let storage = Storage::with_dir(root).await;
    Universe::open(storage).await
}

#[cfg(not(target_family = "wasm"))]
fn wire_new_note(ui: &AppWindow, state: &Arc<SharedState>) {
    let state = Arc::clone(state);
    let handle = ui.as_weak();
    ui.on_new_note(move || {
        let state = Arc::clone(&state);
        let ui = handle.clone();
        tokio::spawn(async move {
            new_note_task(state, ui).await;
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
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_status(format!("universe ready - {} notes", count).into());
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

fn report_boot_failure(ui: slint::Weak<AppWindow>, message: &str) {
    let text = format!("could not open universe: {}", message);
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.set_status(text.into());
        }
    });
}
