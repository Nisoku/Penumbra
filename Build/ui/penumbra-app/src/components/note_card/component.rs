use dioxus::prelude::*;
use dioxus_icons::lucide::Pin;
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

#[css_module("/src/components/note_card/style.css")]
struct Styles;

/// Strip HTML tags for use in the plain-text card preview.
fn strip_html(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => r.push(c),
            _ => {}
        }
    }
    // Collapse multiple whitespace / newlines to single space.
    let mut out = String::with_capacity(r.len());
    let mut prev_ws = false;
    for c in r.chars() {
        if c.is_whitespace() {
            if !prev_ws { out.push(' '); }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

#[component]
pub fn NoteCard(
    title: String,
    preview: String,
    tags: Vec<String>,
    pinned: bool,
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

    let plain_preview = strip_html(&preview);
    let truncated: String = plain_preview.chars().take(90).collect();
    let display_preview = if plain_preview.len() > 90 {
        format!("{truncated}…")
    } else {
        truncated
    };

    let display_title = if title.trim().is_empty() { "New Note" } else { title.trim() };

    rsx! {
        div { ..merged,
            div { class: Styles::dx_note_card_header,
                div { class: Styles::dx_note_card_title,
                    { display_title }
                }
                if pinned {
                    span { class: Styles::dx_note_card_pin,
                        Pin { size: 12, stroke: "currentColor" }
                    }
                }
            }
            if !display_preview.is_empty() {
                div { class: Styles::dx_note_card_preview, "{display_preview}" }
            }
            if !tags.is_empty() {
                div { class: Styles::dx_note_card_tags,
                    for tag in &tags {
                        span { class: Styles::dx_note_card_tag, "{tag}" }
                    }
                }
            }
        }
    }
}
