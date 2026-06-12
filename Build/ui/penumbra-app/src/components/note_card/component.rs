use dioxus::prelude::*;
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

use penumbra_markdown::parser::markdown_to_html;

#[css_module("/src/components/note_card/style.css")]
struct Styles;

#[component]
pub fn NoteCard(
    title: String,
    preview: String,
    x: f64,
    y: f64,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let style = format!("transform: translate({x}px, {y}px)");
    let base = attributes!(div {
        class: Styles::dx_note_card,
        style: style,
    });
    let merged = merge_attributes(vec![base, attributes]);

    let rendered = markdown_to_html(&preview).unwrap_or_else(|_| preview.clone());

    rsx! {
        div { ..merged,
            div { class: Styles::dx_note_card_title, "{title}" }
            div { class: Styles::dx_note_card_preview, dangerous_inner_html: "{rendered}" }
        }
    }
}
