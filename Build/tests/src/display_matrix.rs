use penumbra_editor::session::EditorSession;
use penumbra_ui::display;

fn blocks_of(raw: &str) -> Vec<penumbra_editor::session::BlockEdit> {
    EditorSession::new(raw).blocks().to_vec()
}

#[test]
fn heading_strips_markers_and_inline() {
    let blocks = blocks_of("## My **bold** idea");
    assert_eq!(display::display_text(&blocks[0]), "My bold idea");
    assert_eq!(display::heading_level(&blocks[0]), 2);
    assert_eq!(display::kind_name(&blocks[0].kind), "heading");
}

#[test]
fn heading_strips_hash_run_any_length() {
    let blocks = blocks_of("###### deepest");
    assert_eq!(display::display_text(&blocks[0]), "deepest");
    assert_eq!(display::heading_level(&blocks[0]), 6);
}

#[test]
fn link_in_heading_reduces_to_label() {
    let blocks = blocks_of("# read [this](https://x.dev) now");
    assert_eq!(display::display_text(&blocks[0]), "read this now");
}

#[test]
fn quote_strips_prefix_per_line() {
    let blocks = blocks_of("> first\n> second");
    assert_eq!(display::display_text(&blocks[0]), "first\nsecond");
}

#[test]
fn code_fences_and_language_extracted() {
    let blocks = blocks_of("```rust\nfn main() {}\n```");
    assert_eq!(display::code_language(&blocks[0]), "rust");
    assert_eq!(display::display_text(&blocks[0]), "fn main() {}");
}

#[test]
fn indented_code_has_no_language() {
    let blocks = blocks_of("    let x = 1;");
    assert_eq!(display::kind_name(&blocks[0].kind), "code");
    assert_eq!(display::code_language(&blocks[0]), "");
    assert!(!display::display_text(&blocks[0]).starts_with('`'));
}

#[test]
fn list_keeps_markers_for_markdown_renderer() {
    let blocks = blocks_of("- one\n- two");
    assert_eq!(display::display_text(&blocks[0]), "- one\n- two");
}

#[test]
fn table_renders_aligned_grid() {
    let blocks = blocks_of("| a | b |\n|---|---|\n| 1 | 2 |");
    assert_eq!(
        display::display_text(&blocks[0]),
        "a   | b  \n--- | ---\n1   | 2  "
    );
}

#[test]
fn footnote_strips_definition_label() {
    let blocks = blocks_of("[^1]: see the appendix");
    assert_eq!(display::display_text(&blocks[0]), "see the appendix");
}

#[test]
fn non_block_input_is_passthrough() {
    let blocks = blocks_of("plain prose with a [link](https://x.dev)");
    assert_eq!(display::kind_name(&blocks[0].kind), "paragraph");
    assert_eq!(
        display::display_text(&blocks[0]),
        "plain prose with a [link](https://x.dev)"
    );
}

#[test]
fn scroll_target_tracks_active_block() {
    let mut raw = String::from("# Heading\n\n");
    for _ in 0..30 {
        raw.push_str("This is a fairly long line of body text that wraps across the column.\n");
    }
    let blocks = blocks_of(&raw);
    let early = display::scroll_target_y(&blocks, 0, 600.0, 400.0);
    let late = display::scroll_target_y(&blocks, blocks.len() - 1, 600.0, 400.0);
    assert!(late > early);
    let mid = display::scroll_target_y(&blocks, blocks.len() / 2, 600.0, 400.0);
    assert!(mid > 0.0);
}

#[test]
fn scroll_target_zero_for_first_block() {
    let blocks = blocks_of("# Only one block");
    assert_eq!(display::scroll_target_y(&blocks, 0, 600.0, 400.0), 0.0);
}

#[test]
fn live_kind_tracks_prefix_while_typing() {
    let blocks = blocks_of("plain start");
    assert_eq!(
        display::live_kind_name(&blocks[0].kind, "# New title"),
        "heading"
    );
    assert_eq!(
        display::live_kind_name(&blocks[0].kind, "> quoted line"),
        "quote"
    );
    assert_eq!(display::live_kind_name(&blocks[0].kind, "- one"), "list");
    assert_eq!(display::live_kind_name(&blocks[0].kind, "1. two"), "list");
    assert_eq!(
        display::live_kind_name(&blocks[0].kind, "just prose"),
        "paragraph"
    );
}

#[test]
fn live_display_strips_markers_for_typed_heading_and_quote() {
    let blocks = blocks_of("plain start");
    assert_eq!(
        display::live_display_text(&blocks[0].kind, "# Live **bold**"),
        "Live bold"
    );
    assert_eq!(
        display::live_display_text(&blocks[0].kind, "> one\n> two"),
        "one\ntwo"
    );
    assert_eq!(
        display::live_display_text(&blocks[0].kind, "- a\n- b"),
        "- a\n- b"
    );
}

#[test]
fn live_heading_level_counts_hashes() {
    let blocks = blocks_of("plain start");
    assert_eq!(display::live_heading_level(&blocks[0].kind, "### Three"), 3);
}

#[test]
fn live_height_grows_with_preview_and_clamps_space_for_code() {
    let blocks = blocks_of("```rust\nfn main() {}\n```");
    let empty = display::estimate_live_height(&blocks[0].kind, "", 600.0);
    let filled = display::estimate_live_height(&blocks[0].kind, "```rust\nlet a = 1;\n```", 600.0);
    assert!(filled > empty);
    assert!(empty >= 168.0 + 32.0);
}

#[test]
fn live_preview_collapses_for_breaker_kinds() {
    let blocks = blocks_of("plain");
    assert_eq!(display::live_display_text(&blocks[0].kind, ""), "");
}
