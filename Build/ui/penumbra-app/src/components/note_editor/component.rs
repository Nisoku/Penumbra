use std::sync::Arc;

use dioxus::prelude::*;

use penumbra_core::note::NoteId;

use crate::state::AppState;

#[css_module("/src/components/note_editor/style.css")]
struct Styles;

#[component]
pub fn NoteEditor(
    app_state: Signal<Option<Arc<AppState>>>,
    note_id: Option<NoteId>,
    on_back: EventHandler<MouseEvent>,
) -> Element {
    let mut title = use_signal(String::new);
    let mut body = use_signal(String::new);
    let mut tags = use_signal(String::new);
    let saving = use_signal(|| false);
    let error = use_signal(|| None::<String>);

    // Load existing note or start fresh
    use_effect(move || {
        if let Some(nid) = note_id {
            if let Some(state) = app_state.read().as_ref() {
                if let Ok(notes) = state.all_notes() {
                    if let Some(note) = notes.iter().find(|n| n.id == nid) {
                        title.set(note.title.clone());
                        body.set(note.body.clone());
                        tags.set(note.tags.join(", "));
                        return;
                    }
                }
            }
        }
        // New note is already empty from initial signal values
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
                            state
                                .update_note(&nid, Some(t), Some(b), Some(tag_list))
                                .await
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

    rsx! {
        div { class: Styles::dx_editor,
            div { class: Styles::dx_editor_toolbar,
                button {
                    class: Styles::dx_editor_back,
                    onclick: on_click_back,
                    "\u{2190}  Notes"
                }
                div { class: Styles::dx_editor_status,
                    if saving() {
                        "Saving..."
                    } else if error().is_some() {
                        "Save failed"
                    } else {
                        ""
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
                    textarea {
                        class: Styles::dx_editor_body,
                        value: "{body}",
                        placeholder: "Start writing...",
                        oninput: move |e| body.set(e.value()),
                    }
                    input {
                        class: Styles::dx_editor_tags,
                        value: "{tags}",
                        placeholder: "tag1, tag2, tag3",
                        oninput: move |e| tags.set(e.value()),
                    }
                    {error().map(|msg| rsx! {
                        div { class: Styles::dx_editor_error, "{msg}" }
                    })}
                }
            }
        }
    }
}
