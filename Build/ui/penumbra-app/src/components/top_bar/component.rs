use dioxus::prelude::*;
use dioxus_icons::lucide::Search;
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

#[css_module("/src/components/top_bar/style.css")]
struct Styles;

#[component]
pub fn TopBar(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
    let base = attributes!(div {
        class: Styles::dx_top_bar,
    });
    let merged = merge_attributes(vec![base, attributes]);

    rsx! {
        div { ..merged,
            span { class: Styles::dx_top_bar_name, "Penumbra" }
            div { class: Styles::dx_top_bar_search,
                Search { size: 14, stroke: "currentColor" }
                span { "Search your notes..." }
            }
        }
    }
}
