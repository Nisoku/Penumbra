use dioxus::prelude::*;

fn app() -> Element {
    rsx! {
        div { "Penumbra - Spatial Notes" }
    }
}

pub fn run() {
    dioxus::launch(app);
}