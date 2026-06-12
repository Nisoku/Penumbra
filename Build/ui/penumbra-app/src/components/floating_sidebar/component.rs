use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_icons::lucide::{LayoutGrid, Pin, Search, Settings, SquarePen, Tag};
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

fn title_or_untitled(title: &str) -> &str {
    if title.trim().is_empty() { "Untitled" } else { title.trim() }
}

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
    r
}

#[component]
pub fn FloatingSidebar(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
    on_tag_filter: EventHandler<Option<String>>,
    on_open_editor: EventHandler<NoteId>,
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
                title: "All notes",
                LayoutGrid { size: 15, stroke: "currentColor" }
            }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Search { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Search),
                title: "Search",
                Search { size: 15, stroke: "currentColor" }
            }
            div { class: Styles::dx_floating_sidebar_divider }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Pin { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Pin),
                title: "Pinned notes",
                Pin { size: 15, stroke: "currentColor" }
            }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-active": if active() == SidebarTab::Tag { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Tag),
                title: "Tags",
                Tag { size: 15, stroke: "currentColor" }
            }
            div { class: Styles::dx_floating_sidebar_divider }
            button {
                class: Styles::dx_floating_sidebar_btn,
                "data-spacer": "true",
                "data-active": if active() == SidebarTab::Settings { "true" } else { "false" },
                onclick: move |_| active.set(SidebarTab::Settings),
                title: "Settings",
                Settings { size: 15, stroke: "currentColor" }
            }
        }
        div { class: Styles::dx_sidebar_panel,
            match active() {
                SidebarTab::Grid => rsx! {
                    GridPanel { app_state, on_note_selected, on_open_editor }
                },
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
            }
        }
    }
}

// Grid Panel: list of all notes
#[component]
fn GridPanel(
    app_state: Signal<Option<Arc<AppState>>>,
    on_note_selected: EventHandler<NoteId>,
    on_open_editor: EventHandler<NoteId>,
) -> Element {
    let mut filter = use_signal(String::new);
    let mut sort_by = use_signal(|| "updated");

    let mut notes: Vec<Note> = if let Some(s) = app_state.read().as_ref() {
        s.all_notes().unwrap_or_default()
    } else {
        Vec::new()
    };

    let ft = filter();
    if !ft.trim().is_empty() {
        let q = ft.to_lowercase();
        notes.retain(|n| {
            n.title.to_lowercase().contains(&q)
                || strip_html(&n.body).to_lowercase().contains(&q)
                || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
        });
    }

    match sort_by().as_ref() {
        "title" => notes.sort_by(|a, b| a.title.cmp(&b.title)),
        "created" => notes.sort_by(|a, b| b.meta.created_at.cmp(&a.meta.created_at)),
        _ => notes.sort_by(|a, b| b.meta.updated_at.cmp(&a.meta.updated_at)),
    }

    let count = notes.len();

    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header,
                "All Notes"
                span { class: Styles::dx_note_count, " {count}" }
            }
            input {
                class: Styles::dx_search_input,
                placeholder: "Filter notes...",
                value: "{filter}",
                oninput: move |e| filter.set(e.value()),
            }
            div { class: Styles::dx_sort_row,
                button {
                    class: Styles::dx_sort_btn,
                    "data-active": if sort_by() == "updated" { "true" } else { "false" },
                    onclick: move |_| sort_by.set("updated"),
                    "Recent"
                }
                button {
                    class: Styles::dx_sort_btn,
                    "data-active": if sort_by() == "title" { "true" } else { "false" },
                    onclick: move |_| sort_by.set("title"),
                    "A–Z"
                }
                button {
                    class: Styles::dx_sort_btn,
                    "data-active": if sort_by() == "created" { "true" } else { "false" },
                    onclick: move |_| sort_by.set("created"),
                    "Created"
                }
            }
            div { class: Styles::dx_note_list,
                if notes.is_empty() {
                    div { class: Styles::dx_search_empty, "No notes yet" }
                }
                for note in notes {
                    div { class: Styles::dx_note_list_item,
                        div {
                            class: Styles::dx_note_list_info,
                            onclick: move |_| on_note_selected.call(note.id),
                            div { class: Styles::dx_search_result_title,
                                { title_or_untitled(&note.title) }
                            }
                            {
                                let preview = {
                                    let plain = strip_html(&note.body);
                                    let s: String = plain.chars().take(72).collect();
                                    if plain.len() > 72 { format!("{s}…") } else { s }
                                };
                                rsx! {
                                    div { class: Styles::dx_search_result_preview, "{preview}" }
                                }
                            }
                            if !note.tags.is_empty() {
                                div { class: Styles::dx_note_tags_row,
                                    for tag in &note.tags {
                                        span { class: Styles::dx_note_tag_chip, "{tag}" }
                                    }
                                }
                            }
                        }
                        button {
                            class: Styles::dx_note_edit_btn,
                            title: "Open in editor",
                            onclick: move |_| on_open_editor.call(note.id),
                            SquarePen { size: 14, stroke: "currentColor" }
                        }
                    }
                }
            }
        }
    }
}

// Search Panel
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
                            { title_or_untitled(&r.note.title) }
                        }
                        {
                            let preview = {
                                let plain = strip_html(&r.note.body);
                                let s: String = plain.chars().take(80).collect();
                                if plain.len() > 80 { format!("{s}…") } else { s }
                            };
                            rsx! { div { class: Styles::dx_search_result_preview, "{preview}" } }
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

// Tag Panel
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
                let filtered: Vec<Note> = s
                    .all_notes()
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
                            div { class: Styles::dx_search_result_title,
                                { title_or_untitled(&note.title) }
                            }
                        }
                    }
                }
            }
        }
    }
}

// Pin Panel
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
                    div { class: Styles::dx_search_result_title,
                        { title_or_untitled(&note.title) }
                    }
                }
            }
        }
    }
}

// Settings Panel
#[component]
fn SettingsPanel(app_state: Signal<Option<Arc<AppState>>>) -> Element {
    let mut theme_mode = use_context::<Signal<String>>();

    let mut note_count = use_signal(|| 0usize);
    let mut link_count = use_signal(|| 0usize);
    let mut tag_count = use_signal(|| 0usize);

    let _ = use_effect(move || {
        if let Some(s) = app_state.read().as_ref() {
            if let Ok(notes) = s.all_notes() {
                let lc = s.all_links().map(|l| l.len()).unwrap_or(0);
                let tags: std::collections::HashSet<_> =
                    notes.iter().flat_map(|n| n.tags.iter().cloned()).collect();
                note_count.set(notes.len());
                link_count.set(lc);
                tag_count.set(tags.len());
            }
        }
    });

    rsx! {
        div { class: Styles::dx_sidebar_panel_inner,
            div { class: Styles::dx_panel_header, "Settings" }

            div { class: Styles::dx_settings_section_label, "Theme" }
            div { class: Styles::dx_theme_toggle,
                button {
                    class: Styles::dx_theme_btn,
                    "data-active": if theme_mode() == "dark" { "true" } else { "false" },
                    onclick: move |_| theme_mode.set("dark".into()),
                    "Dark"
                }
                button {
                    class: Styles::dx_theme_btn,
                    "data-active": if theme_mode() == "light" { "true" } else { "false" },
                    onclick: move |_| theme_mode.set("light".into()),
                    "Light"
                }
            }

            div { class: Styles::dx_settings_section_label, "Stats" }
            div { class: Styles::dx_settings_row,
                span { "Notes" }
                span { "{note_count}" }
            }
            div { class: Styles::dx_settings_row,
                span { "Links" }
                span { "{link_count}" }
            }
            div { class: Styles::dx_settings_row,
                span { "Unique tags" }
                span { "{tag_count}" }
            }

            div { class: Styles::dx_settings_section_label, "About" }
            div { class: Styles::dx_settings_about,
                "Penumbra: spatial notes"
            }
        }
    }
}
