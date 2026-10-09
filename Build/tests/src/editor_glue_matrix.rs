use penumbra_editor::session::EditorSession;
use penumbra_ui::editor_block_rows;
use penumbra_ui::BlockVM;

fn rows_of(raw: &str) -> Vec<BlockVM> {
    editor_block_rows(&EditorSession::new(raw)).0
}

#[test]
fn active_index_matches_session_and_flags_row() {
    let mut session = EditorSession::new("# Title\n\nBody text");
    session.set_active(1);
    let (rows, active) = editor_block_rows(&session);
    assert_eq!(active, 1);
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].is_active);
    assert!(rows[1].is_active);
    assert_eq!(rows[1].id, 1);
}

#[test]
fn rows_carry_kind_level_and_language() {
    let rows = rows_of("## Heading\n\n> quote\n\n```rust\nfn main() {}\n```");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind.as_str(), "heading");
    assert_eq!(rows[0].level, 2);
    assert_eq!(rows[1].kind.as_str(), "quote");
    assert_eq!(rows[2].kind.as_str(), "code");
    assert_eq!(rows[2].language.as_str(), "rust");
}

#[test]
fn heading_display_text_strips_markers() {
    let rows = rows_of("# My **bold** title");
    assert_eq!(rows[0].kind.as_str(), "heading");
    assert_eq!(rows[0].level, 1);
    assert_eq!(rows[0].text.as_str(), "My bold title");
    assert_eq!(rows[0].edit_text.as_str(), "# My **bold** title");
}

#[test]
fn html_block_keeps_source_in_display_and_edit() {
    let source = "<div class=\"row\">\n<p>hi</p>\n</div>";
    let rows = rows_of(source);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind.as_str(), "html");
    assert_eq!(rows[0].text.as_str(), source);
    assert_eq!(rows[0].edit_text.as_str(), source);
}

#[test]
fn break_block_flows_through_rows() {
    let rows = rows_of("above\n\n---\n\nbelow");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].kind.as_str(), "break");
    assert_eq!(rows[1].edit_text.as_str(), "---");
}

#[test]
fn typed_marker_re_derives_kind_but_edit_text_keeps_source() {
    let mut session = EditorSession::new("plain");
    session.apply_active_text("# becoming heading");
    let (rows, active) = editor_block_rows(&session);
    assert_eq!(active, 0);
    assert_eq!(rows[0].kind.as_str(), "heading");
    assert_eq!(rows[0].level, 1);
    assert_eq!(rows[0].edit_text.as_str(), "# becoming heading");
}
