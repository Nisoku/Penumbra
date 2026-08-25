use std::sync::Arc;

use penumbra_app::Universe;
use slint::ComponentHandle;
use tokio::sync::Mutex as AsyncMutex;

slint::include_modules!();

const WELCOME_TITLE: &str = "Welcome to Penumbra";
const WELCOME_BODY: &str = "This is your first note.\n\nEverything you write lands on the map over time. Drag notes around, link them, and watch constellations form.";

struct SharedState {
    universe: AsyncMutex<Option<Universe>>,
    io: tokio::runtime::Handle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let state = Arc::new(SharedState {
        universe: AsyncMutex::new(None),
        io: runtime.handle().clone(),
    });

    wire_new_note(&ui, &state);

    let boot_state = Arc::clone(&state);
    let boot_ui = ui.as_weak();
    runtime.spawn(async move {
        match Universe::open_default().await {
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
                *boot_state.universe.lock().await = Some(universe);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = boot_ui.upgrade() {
                        ui.set_ready(true);
                        ui.set_status(format!("universe ready - {} notes", count).into());
                    }
                });
            }
            Err(err) => report_boot_failure(boot_ui, &err.to_string()),
        }
    });

    ui.run()?;
    drop(runtime);
    Ok(())
}

fn wire_new_note(ui: &AppWindow, state: &Arc<SharedState>) {
    let state = Arc::clone(state);
    let handle = ui.as_weak();
    ui.on_new_note(move || {
        let state = Arc::clone(&state);
        let ui = handle.clone();
        let io = state.io.clone();
        io.spawn(async move {
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
        });
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
