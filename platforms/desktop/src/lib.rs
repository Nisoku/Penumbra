use dioxus::prelude::*;

fn app() -> Element {
    rsx! {
        div { "Penumbra - Desktop" }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run() {
    println!("Penumbra Desktop starting...");
    dioxus::launch(app);
}