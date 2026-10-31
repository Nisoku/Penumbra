//! Round-trip tests for the rich editor's markdown serialization.

use penumbra_ui::rich_editor::RichEditor;

#[test]
fn first_render_matches_second_render() {
    let mut editor = RichEditor::new("plain **bold** text\n\n## Heading\n\n`code` span");
    let first = editor.surface_rgba();
    let second = editor.surface_rgba();
    assert_eq!(
        first, second,
        "first render differs from second with no input between them; \
         the first surface shaped incompletely"
    );
}

#[test]
fn formatted_surface_differs_from_plain() {
    let mut rich = RichEditor::new("plain **bold** text\n\n## Heading");
    let mut plain = RichEditor::new("plain bold text\n\nHeading");
    let rich_px = rich.surface_rgba();
    let plain_px = plain.surface_rgba();
    assert_ne!(
        rich_px, plain_px,
        "formatted surface is identical to the unformatted one; \
         rich attributes are not being applied on the first render"
    );
}

/// A paragraph of plain prose survives a construct/serialize round-trip.
#[test]
fn plain_paragraph_round_trips() {
    let editor = RichEditor::new("Hello world.");
    assert_eq!(editor.to_markdown(), "Hello world.");
}

/// Two paragraphs stay separated by a blank line.
#[test]
fn two_paragraphs_keep_separation() {
    let editor = RichEditor::new("First paragraph.\n\nSecond paragraph.");
    assert_eq!(
        editor.to_markdown(),
        "First paragraph.\n\nSecond paragraph."
    );
}

/// Typing printable characters appends them to the serialized document.
#[test]
fn typing_appends_text() {
    let mut editor = RichEditor::new("Start");
    // Caret begins at the document start; move to end, then type.
    editor.input_key("End", false, false, false, false);
    for ch in " more".chars() {
        editor.input_key(&ch.to_string(), false, false, false, false);
    }
    assert!(
        editor.to_markdown().contains("Start more"),
        "serialized markdown was: {:?}",
        editor.to_markdown()
    );
}

/// Inline bold from the source markdown survives a round-trip as `**...**`.
#[test]
fn bold_inline_round_trips() {
    let editor = RichEditor::new("plain **bold** plain");
    let out = editor.to_markdown();
    assert!(
        out.contains("**bold**"),
        "expected bold marker preserved, got: {out:?}"
    );
}

/// Inline code from the source markdown survives a round-trip as `` `...` ``.
#[test]
fn code_inline_round_trips() {
    let editor = RichEditor::new("call `run()` now");
    let out = editor.to_markdown();
    assert!(
        out.contains("`run()`"),
        "expected code span preserved, got: {out:?}"
    );
}

/// An empty document serializes to an empty string, not a panic.
#[test]
fn empty_document_serializes_empty() {
    let editor = RichEditor::new("");
    assert_eq!(editor.to_markdown(), "");
}

#[test]
fn strikethrough_inline_round_trips() {
    let editor = RichEditor::new("keep ~~gone~~ keep");
    let out = editor.to_markdown();
    assert!(
        out.contains("~~gone~~"),
        "expected strikethrough preserved, got: {out:?}"
    );
}

#[test]
fn ordered_list_round_trips() {
    let editor = RichEditor::new("1. first\n2. second\n3. third");
    let out = editor.to_markdown();
    for marker in ["1. first", "2. second", "3. third"] {
        assert!(
            out.contains(marker),
            "expected ordered marker {marker:?}, got: {out:?}"
        );
    }
}

#[test]
fn heading_levels_render_at_distinct_sizes() {
    let mut h1 = RichEditor::new("# Observatory");
    let mut h3 = RichEditor::new("### Observatory");
    assert_ne!(
        h1.surface_rgba(),
        h3.surface_rgba(),
        "H1 and H3 produced identical surfaces; heading sizes are not varying"
    );
}

#[test]
fn quote_renders_distinct_from_paragraph() {
    let mut quote = RichEditor::new("> remembered weather");
    let mut plain = RichEditor::new("remembered weather");
    assert_ne!(
        quote.surface_rgba(),
        plain.surface_rgba(),
        "quote surface matches plain paragraph; quote styling is not applied"
    );
}

#[test]
fn table_renders_distinct_from_plain_text() {
    let mut table = RichEditor::new("| a | b |\n| - | - |\n| 1 | 2 |");
    let mut plain = RichEditor::new("a b 1 2");
    assert_ne!(
        table.surface_rgba(),
        plain.surface_rgba(),
        "table surface matches plain text; grid rendering is not applied"
    );
}

const PADDING_X: f32 = 30.0;
const PADDING_TOP: f32 = 22.0;
const HALF_LINE: f32 = 12.0;

#[test]
fn click_at_glyph_origin_lands_at_line_start() {
    let mut editor = RichEditor::new("first second third");
    editor.surface();
    // Surface x at the glyph origin, y mid-first-line.
    editor.pointer_down(PADDING_X + 1.0, PADDING_TOP + HALF_LINE, false);
    assert!(
        editor.status().contains("caret: line 1 byte 0"),
        "expected caret at line start, got: {}",
        editor.status()
    );
}

#[test]
fn click_past_end_of_line_clamps_to_line_end() {
    let text = "first second third";
    let mut editor = RichEditor::new(text);
    editor.surface();
    editor.pointer_down(PADDING_X + 10_000.0, PADDING_TOP + HALF_LINE, false);
    let expected = format!("caret: line 1 byte {}", text.len());
    assert!(
        editor.status().contains(&expected),
        "expected caret clamped to line end ({expected}), got: {}",
        editor.status()
    );
}

#[test]
fn double_click_selects_word() {
    let mut editor = RichEditor::new("alpha beta gamma");
    editor.surface();
    // Aim near the middle of the first word, allowing for the padding offset.
    editor.pointer_double_click(PADDING_X + 12.0, PADDING_TOP + HALF_LINE);
    let status = editor.status();
    assert!(
        status.contains("Selection: line 1 byte 0 -> line 1 byte 5"),
        "expected first word selected, got: {status}"
    );
}

/// Triple-click selects the whole visual line/paragraph under the pointer.
#[test]
fn triple_click_selects_line() {
    let text = "the quick brown fox";
    let mut editor = RichEditor::new(text);
    editor.surface();
    editor.pointer_triple_click(PADDING_X + 12.0, PADDING_TOP + HALF_LINE);
    let status = editor.status();
    let expected = format!("Selection: line 1 byte 0 -> line 1 byte {}", text.len());
    assert!(
        status.contains(&expected),
        "expected whole line selected ({expected}), got: {status}"
    );
}

/// Shift+click extends the existing caret into a selection rather than
/// collapsing to the clicked point.
#[test]
fn shift_click_extends_selection() {
    let mut editor = RichEditor::new("alpha beta gamma");
    editor.surface();
    // Place the caret at the line start.
    editor.pointer_down(PADDING_X + 1.0, PADDING_TOP + HALF_LINE, false);
    // Shift+click further right extends a selection from the caret.
    editor.pointer_down(PADDING_X + 10_000.0, PADDING_TOP + HALF_LINE, true);
    let status = editor.status();
    assert!(
        status.starts_with("Selection: line 1 byte 0 ->"),
        "expected extended selection from line start, got: {status}"
    );
}
