use dioxus::prelude::*;
use dioxus_icons::lucide::{LayoutGrid, Pin, Search, Settings, Tag};
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

#[css_module("/src/components/floating_sidebar/style.css")]
struct Styles;

#[derive(Clone, Copy, PartialEq)]
enum SidebarTab {
    Grid,
    Search,
    Pin,
    Tag,
    Settings,
}

#[component]
pub fn FloatingSidebar(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
    let mut active = use_signal(|| SidebarTab::Grid);

    let base = attributes!(div {
        class: Styles::dx_floating_sidebar,
    });
    let merged = merge_attributes(vec![base, attributes]);

    rsx! {
        div { ..merged,
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Grid { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Grid),
                LayoutGrid { size: 15, stroke: "currentColor" }
            }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Search { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Search),
                Search { size: 15, stroke: "currentColor" }
            }
            div { class: Styles::dx_floating_sidebar_divider }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Pin { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Pin),
                Pin { size: 15, stroke: "currentColor" }
            }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Tag { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Tag),
                Tag { size: 15, stroke: "currentColor" }
            }
            div { class: Styles::dx_floating_sidebar_divider }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-spacer": "true",
                "data-active": if active() == SidebarTab::Settings { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Settings),
                Settings { size: 15, stroke: "currentColor" }
            }
        }
    }
}
