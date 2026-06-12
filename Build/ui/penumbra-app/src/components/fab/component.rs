use dioxus::prelude::*;
use dioxus_icons::lucide::Plus;
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

#[css_module("/src/components/fab/style.css")]
struct Styles;

#[component]
pub fn Fab(
    onclick: Option<EventHandler<MouseEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let base = attributes!(button {
        class: Styles::dx_fab,
    });
    let merged = merge_attributes(vec![base, attributes]);

    rsx! {
        button {
            onclick: move |e| {
                if let Some(f) = &onclick {
                    f.call(e);
                }
            },
            ..merged,
            Plus { size: 15, stroke: "currentColor" }
            "New note"
        }
    }
}
