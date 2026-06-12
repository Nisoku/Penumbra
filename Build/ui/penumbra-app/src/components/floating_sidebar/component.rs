use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_icons::lucide::{LayoutGrid, Pin, Search, Settings, Tag};
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

use penumbra_core::note::{Note, NoteId};
use penumbra_search::SearchResult;

use crate::state::AppState;

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
pub fn FloatingSidebar(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
    on_tag_filter: EventHandler<Option<String>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
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
        if active() != SidebarTab::Grid {
            div { class: Styles::dx_sidebar_panel,
                match active() {
                    SidebarTab::Search => rsx! {
                        SearchPanel { app_state, on_note_selected }
                    },
                    SidebarTab::Tag => rsx! {
                        TagPanel { app_state, on_note_selected, on_tag_filter }
                    },
                    SidebarTab::Pin => rsx! {
                        PinPanel { app_state, on_note_selected }
                    },
                    SidebarTab::Settings => rsx! {
                        SettingsPanel { app_state }
                    },
                    _ => rsx! {},
                }
            }
        }
    }
}

#[component]
fn SearchPanel(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
) -> Element {
    let mut query = use_signal(String::new);
    let mut results = use_signal(Vec::<SearchResult>::new);
    let mut searching = use_signal(|| false);

    use_effect(move || {
        let q = query();
        if q.trim().is_empty() {
            results.set(Vec::new());
            searching.set(false);
            return;
        }
        searching.set(true);
        let state = app_state;
        spawn(async move {
            let r = match state.read().as_ref() {
                Some(s) => s.search(&q, &[]).await.unwrap_or_default(),
                None => Vec::new(),
            };
            results.set(r);
            searching.set(false);
        });
    });

    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header, "Search" }
            input {
                class: Styles::dx_search_input,
                placeholder: "Search notes...",
                value: "{query}",
                autofocus: "true",
                oninput: move |e| query.set(e.value()),
            }
            if searching() {
                div { class: Styles::dx_search_status, "Searching..." }
            }
            div { class: Styles::dx_search_results,
                for r in results() {
                    div {
                        class: Styles::dx_search_result,
                        onclick: move |_| on_note_selected.call(r.note_id),
                        div { class: Styles::dx_search_result_title,
                            {r.note.title}
                        }
                        div { class: Styles::dx_search_result_preview,
                            {if r.note.body.len() > 80 { &r.note.body[..80] } else { &r.note.body }}
                        }
                        div { class: Styles::dx_search_result_meta,
                            span { "Score: {r.score:.2}" }
                        }
                    }
                }
                if results().is_empty() && !query().trim().is_empty() && !searching() {
                    div { class: Styles::dx_search_empty, "No results" }
                }
            }
        }
    }
}

#[component]
fn TagPanel(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
    on_tag_filter: EventHandler<Option<String>>,
) -> Element {
    let mut tag_counts = use_signal(HashMap::<String, usize>::new);
    let selected_tag = use_signal(|| None::<String>);
    let mut tagged_notes = use_signal(Vec::<Note>::new);

    use_effect(move || {
        if let Some(s) = app_state.read().as_ref() {
            let notes = s.all_notes().unwrap_or_default();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for note in &notes {
                for tag in &note.tags {
                    *counts.entry(tag.clone()).or_insert(0) += 1;
                }
            }
            tag_counts.set(counts);
        }
    });

    use_effect(move || {
        let tag = selected_tag();
        if let Some(t) = tag {
            if let Some(s) = app_state.read().as_ref() {
                    let filtered: Vec<Note> = s.all_notes()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|n| n.tags.contains(&t))
                        .collect();
                tagged_notes.set(filtered);
            }
        } else {
            tagged_notes.set(Vec::new());
        }
    });

    let tag_list: Vec<(String, usize)> = {
        let mut v: Vec<(String, usize)> = tag_counts().into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    };
    let has_tags = !tag_list.is_empty();

    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header, "Tags" }
            div { class: Styles::dx_tag_list,
                if !has_tags {
                    div { class: Styles::dx_search_empty, "No tags yet" }
                }
                for (tag, count) in tag_list {
                    button {
                        class: Styles::dx_tag_item,
                        "data-active": if selected_tag() == Some(tag.clone()) { "true" } else { "false" },
                        onclick: {
                            let mut sel = selected_tag;
                            let filter = on_tag_filter;
                            move |_| {
                                if sel() == Some(tag.clone()) {
                                    sel.set(None);
                                    filter.call(None);
                                } else {
                                    sel.set(Some(tag.clone()));
                                    filter.call(Some(tag.clone()));
                                }
                            }
                        },
                        span { "{tag}" }
                        span { class: Styles::dx_tag_count, "{count}" }
                    }
                }
            }
            if !tagged_notes().is_empty() {
                div { class: Styles::dx_panel_section,
                    div { class: Styles::dx_panel_section_title, "Tagged notes" }
                    for note in tagged_notes() {
                        div {
                            class: Styles::dx_tag_note_item,
                            onclick: move |_| on_note_selected.call(note.id),
                            div { class: Styles::dx_search_result_title, "{note.title}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PinPanel(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
) -> Element {
    let mut pinned = use_signal(Vec::<Note>::new);

    use_effect(move || {
        if let Some(s) = app_state.read().as_ref() {
            let notes = s.all_notes().unwrap_or_default();
            pinned.set(notes.into_iter().filter(|n| n.meta.pinned).collect());
        }
    });

    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header, "Pinned" }
            if pinned().is_empty() {
                div { class: Styles::dx_search_empty, "No pinned notes" }
            }
            for note in pinned() {
                div {
                    class: Styles::dx_search_result,
                    onclick: move |_| on_note_selected.call(note.id),
                    div { class: Styles::dx_search_result_title, "{note.title}" }
                }
            }
        }
    }
}

#[component]
fn SettingsPanel(
    app_state: Signal<Option<Arc<AppState>>>,
) -> Element {
    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header, "Settings" }
            div { class: Styles::dx_settings_row,
                span { "Notes: " }
                span { "{app_state.read().as_ref().map(|s| s.all_notes().unwrap_or_default().len()).unwrap_or(0)}" }
            }
            div { class: Styles::dx_settings_row,
                span { "Links: " }
                span { "{app_state.read().as_ref().map(|s| s.all_links().unwrap_or_default().len()).unwrap_or(0)}" }
            }
        }
    }
}
