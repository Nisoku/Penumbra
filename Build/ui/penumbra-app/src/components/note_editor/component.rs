use std::sync::Arc;

use dioxus::document::eval;
use dioxus::prelude::*;
use dioxus_icons::lucide::{ArrowLeft, Trash2};

use penumbra_core::note::NoteId;

use crate::state::AppState;

#[css_module("/src/components/note_editor/style.css")]
struct Styles;

const TIPTAP_JS: &str = include_str!("../../../js/tiptap-editor.js");

/// Count words in an HTML body by stripping tags first.
fn word_count(html: &str) -> usize {
    let mut plain = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(c),
            _ => {}
        }
    }
    plain.split_whitespace().filter(|w| !w.is_empty()).count()
}

fn is_body_html(body: &str) -> bool {
    let trimmed = body.trim();
    trimmed.starts_with('<')
        && (trimmed.contains("<p>")
            || trimmed.contains("<h")
            || trimmed.contains("<ul")
            || trimmed.contains("<ol")
            || trimmed.contains("<div")
            || trimmed.contains("<pre")
            || trimmed.contains("<blockquote"))
}

#[component]
pub fn NoteEditor(
    app_state: Signal<Option<Arc<AppState>>>,
    note_id: Option<NoteId>,
    on_back: EventHandler<MouseEvent>,
    on_delete: EventHandler<()>,
) -> Element {
    let mut title = use_signal(String::new);
    let mut body = use_signal(String::new);
    let mut tags = use_signal(String::new);
    let saving = use_signal(|| false);
    let error = use_signal(|| None::<String>);
    let mut tiptap_error = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);
    let mut delete_confirm = use_signal(|| false);

    // Load existing note data (synchronous as graph is in memory).
    use_effect(move || {
        if let Some(nid) = note_id {
            if let Some(state) = app_state.read().as_ref() {
                if let Ok(notes) = state.all_notes() {
                    if let Some(note) = notes.iter().find(|n| n.id == nid) {
                        title.set(note.title.clone());
                        body.set(note.body.clone());
                        tags.set(note.tags.join(", "));
                        loaded.set(true);
                        return;
                    }
                }
            }
        }
        loaded.set(true);
    });

    // Start TipTap only after note data is ready. The guard `if !is_loaded { return; }`
    // keeps the first (pre-data) run cheap and prevents double-mounting the editor.
    let _ = use_resource(move || {
        let is_loaded = loaded();
        let initial = if is_loaded {
            let b = body.read();
            if is_body_html(&b) {
                b.clone()
            } else if !b.is_empty() {
                penumbra_markdown::parser::markdown_to_html(&b).unwrap_or(b.clone())
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        async move {
            if !is_loaded {
                return;
            }
            let mut ev = eval(TIPTAP_JS);
            if let Err(e) = ev.send(&initial) {
                tiptap_error.set(Some(format!("eval send failed: {e}")));
                return;
            }
            loop {
                match ev.recv::<String>().await {
                    Ok(msg) => {
                        if msg.starts_with("__ERROR__") {
                            tiptap_error.set(Some(msg));
                            break;
                        }
                        body.set(msg);
                    }
                    Err(e) => {
                        tiptap_error.set(Some(format!("eval recv failed: {e}")));
                        break;
                    }
                }
            }
        }
    });

    let mut do_save = {
        let title = title;
        let body = body;
        let tags = tags;
        let mut saving = saving;
        let mut error = error;
        let app_state = app_state;
        let note_id = note_id;

        move || {
            let t = title();
            let b = body();
            let tag_list: Vec<String> = tags()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let editing = note_id;
            let app = (*app_state.read()).clone();

            if t.trim().is_empty() {
                return;
            }

            saving.set(true);
            error.set(None);

            spawn(async move {
                match app {
                    Some(state) => {
                        let result = if let Some(nid) = editing {
                            state.update_note(&nid, Some(t), Some(b), Some(tag_list)).await
                        } else {
                            state.add_note(t, b).await.map(|_| ())
                        };
                        match result {
                            Ok(()) => error.set(None),
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    }
                    None => error.set(Some("Not initialized".into())),
                }
                saving.set(false);
            });
        }
    };

    let on_click_back = move |e: MouseEvent| {
        do_save();
        on_back.call(e);
    };

    let editor_error = tiptap_error();

    rsx! {
        div { class: Styles::dx_editor,
            div { class: Styles::dx_editor_toolbar,
                button {
                    class: Styles::dx_editor_back,
                    onclick: on_click_back,
                    ArrowLeft { size: 15, stroke: "currentColor" }
                    span { "Notes" }
                }
                div { class: Styles::dx_editor_status,
                    if saving() { "Saving..." }
                    else if error().is_some() { "Save failed" }
                    else { "" }
                }
                if note_id.is_some() {
                    if delete_confirm() {
                        div { class: Styles::dx_editor_delete_confirm,
                            span { "Delete?" }
                            button {
                                class: Styles::dx_editor_btn_confirm_yes,
                                onclick: move |_| {
                                    delete_confirm.set(false);
                                    on_delete.call(());
                                },
                                "Yes"
                            }
                            button {
                                class: Styles::dx_editor_btn_confirm_cancel,
                                onclick: move |_| delete_confirm.set(false),
                                "No"
                            }
                        }
                    } else {
                        button {
                            class: Styles::dx_editor_btn_delete,
                            onclick: move |_| delete_confirm.set(true),
                            Trash2 { size: 13, stroke: "currentColor" }
                            span { "Delete" }
                        }
                    }
                }
            }
            div { class: Styles::dx_editor_content,
                div { class: Styles::dx_editor_inner,
                    input {
                        class: Styles::dx_editor_title,
                        value: "{title}",
                        placeholder: "Untitled",
                        oninput: move |e| title.set(e.value()),
                    }
                    div { id: "penumbra-editor" }
                    input {
                        class: Styles::dx_editor_tags,
                        value: "{tags}",
                        placeholder: "tag1, tag2, tag3",
                        oninput: move |e| tags.set(e.value()),
                    }
                    div { class: Styles::dx_editor_meta,
                        span { "{word_count(&body())} words" }
                    }
                    {error().map(|msg| rsx! {
                        div { class: Styles::dx_editor_error, "{msg}" }
                    })}
                    {editor_error.as_ref().map(|msg| rsx! {
                        div { class: Styles::dx_editor_error, "TipTap: {msg}" }
                    })}
                }
            }
        }
    }
}
