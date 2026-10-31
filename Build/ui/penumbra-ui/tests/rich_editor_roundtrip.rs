//! Round-trip tests for the rich editor's markdown serialization.

use penumbra_ui::rich_editor::RichEditor;

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
    editor.input_key("End", false, false, false);
    for ch in " more".chars() {
        editor.input_key(&ch.to_string(), false, false, false);
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
