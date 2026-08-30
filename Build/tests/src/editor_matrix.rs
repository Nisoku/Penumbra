use std::collections::HashSet;

use penumbra_editor::{
    apply_command, BlockKind, BlockMark, Command, Cursor, Document, EditorSession, Journal,
    Selection, ViewModel,
};

// Document

#[test]
fn doc_parse_paragraph() {
    let doc = Document::new("# Hello\n\nWorld");
    assert_eq!(doc.blocks().len(), 2);
    assert!(matches!(
        doc.blocks()[0].kind,
        BlockKind::Heading { level: 1, .. }
    ));
    assert!(matches!(doc.blocks()[1].kind, BlockKind::Paragraph(_)));
}

#[test]
fn doc_parse_code_block() {
    let doc = Document::new("```\nfoo\n```");
    assert_eq!(doc.blocks().len(), 1);
    assert!(matches!(&doc.blocks()[0].kind, BlockKind::CodeBlock { .. }));
}

#[test]
fn doc_parse_list() {
    let doc = Document::new("1. First\n2. Second");
    assert_eq!(doc.blocks().len(), 1);
    assert!(matches!(
        &doc.blocks()[0].kind,
        BlockKind::List { ordered: true, .. }
    ));
}

#[test]
fn doc_block_ids_unique() {
    let doc = Document::new("# Hello\n\nWorld\n\nMore text");
    let ids: HashSet<_> = doc.blocks().iter().map(|b| b.id).collect();
    assert_eq!(ids.len(), doc.blocks().len());
}

#[test]
fn doc_block_lookup() {
    let doc = Document::new("# Hello\n\nWorld");
    let first_id = doc.blocks()[0].id;
    assert!(doc.block(first_id).is_some());
    assert!(matches!(
        doc.block(first_id).unwrap().kind,
        BlockKind::Heading { level: 1, .. }
    ));
}

#[test]
fn doc_source_range() {
    let doc = Document::new("# Hello\n\nWorld");
    let first_id = doc.blocks()[0].id;
    let second_id = doc.blocks()[1].id;
    assert_eq!(doc.source_range(first_id), Some((0, 9)));
    assert_eq!(doc.source_range(second_id), Some((9, 14)));
}

// Cursor

#[test]
fn cursor_new() {
    let c = Cursor::new(0);
    assert_eq!(c.offset(), 0);
}

#[test]
fn cursor_move_left_right() {
    let mut c = Cursor::new(0);
    c.move_right(10);
    c.move_right(10);
    c.move_right(10);
    assert_eq!(c.offset(), 3);
    c.move_left();
    assert_eq!(c.offset(), 2);
}

#[test]
fn cursor_at_boundaries() {
    let source = "Hello";
    let mut c = Cursor::new(0);
    assert!(c.at_start());
    assert!(!c.at_end(source.len()));
    c.set_offset(source.len());
    assert!(c.at_end(source.len()));
    assert!(!c.at_start());
    c.move_left();
    assert!(!c.at_end(source.len()));
}

// Selection

#[test]
fn selection_collapsed() {
    let s = Selection::new(5, 5);
    assert!(s.collapsed());
}

#[test]
fn selection_range() {
    let s = Selection::new(5, 10);
    assert_eq!(s.range(), (5, 10));
    let s_inv = Selection::new(10, 5);
    assert_eq!(s_inv.range(), (5, 10));
}

#[test]
fn selection_invert() {
    let s = Selection::new(3, 7);
    let inv = s.invert();
    assert_eq!(inv.anchor, 7);
    assert_eq!(inv.head, 3);
}

// Command

#[test]
fn command_insert_text() {
    let cmd = Command::InsertText {
        at: 5,
        text: " World".to_string(),
    };
    let result = apply_command("Hello", &cmd).unwrap();
    assert_eq!(result, "Hello World");
}

#[test]
fn command_delete_range() {
    let cmd = Command::DeleteRange { from: 5, to: 11 };
    let result = apply_command("Hello World", &cmd).unwrap();
    assert_eq!(result, "Hello");
}

#[test]
fn command_split_block() {
    let cmd = Command::SplitBlock { at: 5 };
    let result = apply_command("Hello World", &cmd).unwrap();
    assert_eq!(result, "Hello\n\n World");
}

#[test]
fn command_journal_undo_redo() {
    let mut journal = Journal::new();
    let cmd1 = Command::InsertText {
        at: 0,
        text: "a".to_string(),
    };
    let cmd2 = Command::InsertText {
        at: 1,
        text: "b".to_string(),
    };
    let cmd3 = Command::InsertText {
        at: 2,
        text: "c".to_string(),
    };
    journal.push(cmd1);
    journal.push(cmd2);
    journal.push(cmd3);

    let undone2 = journal.undo();
    assert!(undone2.is_some());
    let undone1 = journal.undo();
    assert!(undone1.is_some());

    let redone = journal.redo();
    assert!(redone.is_some());

    assert!(journal.can_undo());
    assert!(journal.can_redo());
}

#[test]
fn journal_undo_redo_boundary() {
    let mut journal = Journal::new();
    assert!(journal.undo().is_none());
    journal.push(Command::InsertText {
        at: 0,
        text: "x".to_string(),
    });
    let _ = journal.undo();
    assert!(journal.undo().is_none());
}

#[test]
fn journal_redo_boundary() {
    let mut journal = Journal::new();
    assert!(journal.redo().is_none());
    journal.push(Command::InsertText {
        at: 0,
        text: "x".to_string(),
    });
    assert!(journal.redo().is_none());
}

// ViewModel

#[test]
fn view_model_from_doc() {
    let doc = Document::new("# Hello\n\nWorld");
    let vm = ViewModel::from_doc(&doc, None, Cursor::new(0));
    assert_eq!(vm.blocks().len(), 2);
    assert!(matches!(
        &vm.blocks()[0].kind,
        BlockKind::Heading { level: 1, .. }
    ));
    assert!(matches!(&vm.blocks()[1].kind, BlockKind::Paragraph(_)));
}

#[test]
fn view_model_active_block() {
    let doc = Document::new("# Hello\n\nWorld");
    let active_id = doc.blocks()[1].id;
    let vm = ViewModel::from_doc(&doc, Some(active_id), Cursor::new(0));
    assert_eq!(vm.active_block(), Some(active_id));
    assert!(!vm.blocks()[0].is_active);
    assert!(vm.blocks()[1].is_active);
}

#[test]
fn view_model_cursor() {
    let doc = Document::new("# Hello\n\nWorld");
    let vm = ViewModel::from_doc(&doc, None, Cursor::new(5));
    assert_eq!(vm.cursor().offset(), 5);
}

// Integration

#[test]
fn edit_cycle() {
    let source = "Hello World";
    let cmd = Command::InsertText {
        at: 5,
        text: " Beautiful".to_string(),
    };
    let modified = apply_command(source, &cmd).unwrap();
    assert_eq!(modified, "Hello Beautiful World");
    let doc = Document::new(&modified);
    assert_eq!(doc.blocks().len(), 1);
    assert!(matches!(&doc.blocks()[0].kind, BlockKind::Paragraph(_)));
    assert_eq!(doc.source, modified);
}

#[test]
fn bold_across_block() {
    let source = "# Hello\n\nWorld";
    let doc = Document::new(source);
    let block_id = doc.blocks()[0].id;
    let cmd = Command::SetMark {
        block: block_id,
        mark: BlockMark::Bold,
    };
    let result = apply_command(source, &cmd).unwrap();
    assert_eq!(result, source);
}

// EditorSession

#[test]
fn session_splits_paragraph_into_two() {
    let mut session = EditorSession::new("one two three");
    assert_eq!(session.blocks().len(), 1);
    session.split_active_at("one ".len());
    assert_eq!(session.blocks().len(), 2);
    assert_eq!(session.blocks()[0].text, "one");
    assert_eq!(session.blocks()[1].text, "two three");
    assert_eq!(session.active(), 1);
    assert_eq!(session.raw_body(), "one\n\ntwo three");
}

#[test]
fn session_heading_split_drops_tail_into_paragraph() {
    let mut session = EditorSession::new("## Alpha beta");
    session.split_active_at("## Alpha ".len());
    assert_eq!(session.blocks().len(), 2);
    assert!(matches!(
        session.blocks()[0].kind,
        BlockKind::Heading { level: 2, .. }
    ));
    assert!(matches!(session.blocks()[1].kind, BlockKind::Paragraph(_)));
    assert_eq!(session.blocks()[1].text, "beta");
}

#[test]
fn session_list_split_keeps_list_kind() {
    let mut session = EditorSession::new("- alpha\n- beta");
    session.set_active(0);
    session.split_active_at("- al".len());
    assert_eq!(session.blocks().len(), 2);
    assert!(matches!(session.blocks()[0].kind, BlockKind::List { .. }));
    assert!(matches!(session.blocks()[1].kind, BlockKind::List { .. }));
    assert_eq!(session.blocks()[0].text, "- al");
    assert_eq!(session.blocks()[1].text, "pha\n- beta");
    assert_eq!(session.raw_body(), "- al\npha\n- beta");
}

#[test]
fn session_enter_at_block_start_opens_paragraph_above() {
    let mut session = EditorSession::new("hello");
    session.split_active_at(0);
    assert_eq!(session.blocks().len(), 2);
    assert!(matches!(session.blocks()[0].kind, BlockKind::Paragraph(_)));
    assert!(session.blocks()[0].text.is_empty());
    assert_eq!(session.active(), 0);
}

#[test]
fn session_enter_in_empty_block_removes_it() {
    let mut session = EditorSession::new("alpha\n\nbeta");
    assert_eq!(session.blocks().len(), 2);
    session.set_active(1);
    session.split_active_at(0);
    assert_eq!(session.blocks().len(), 3);
    assert_eq!(session.blocks()[1].text, "");
    assert_eq!(session.active(), 1);
    session.apply_active_text("");
    session.split_active_at(0);
    assert_eq!(session.blocks().len(), 2);
}

#[test]
fn session_merge_into_previous_joins_prose_with_space() {
    let mut session = EditorSession::new("alpha\n\nbeta");
    session.set_active(1);
    session.merge_into_previous();
    assert_eq!(session.blocks().len(), 1);
    assert_eq!(session.blocks()[0].text, "alpha beta");
    assert_eq!(session.active(), 0);
}

#[test]
fn session_merge_respects_trailing_whitespace() {
    let mut session = EditorSession::new("prefix  \n\nnext");
    session.set_active(1);
    session.merge_into_previous();
    assert_eq!(session.blocks()[0].text, "prefix  next");
}

#[test]
fn session_merge_list_joins_on_newline() {
    let mut session = EditorSession::new("- a\n- b");
    session.set_active(1);
    session.merge_into_previous();
    assert_eq!(session.blocks().len(), 1);
    assert_eq!(session.blocks()[0].text, "- a\n- b");
}

#[test]
fn session_apply_text_normalizes_blank_lines_in_prose() {
    let mut session = EditorSession::new("alpha");
    session.apply_active_text("line one\n\n\nline two\n");
    assert_eq!(session.blocks()[0].text, "line one\nline two");
    assert_eq!(session.raw_body(), "line one\nline two");
}

#[test]
fn session_code_block_keeps_raw_newlines() {
    let session = EditorSession::new("```rust\nfn main() {}\n```");
    let body = session.blocks()[0].text.clone();
    assert_eq!(session.active_mode(), penumbra_editor::BlockMode::Raw);
    assert_eq!(body, "```rust\nfn main() {}\n```");
}

#[test]
fn session_undo_restores_previous_body() {
    let mut session = EditorSession::new("alpha\n\nbeta");
    session.set_active(1);
    session.apply_active_text("be ta");
    assert_eq!(session.raw_body(), "alpha\n\nbe ta");
    session.undo();
    assert_eq!(session.raw_body(), "alpha\n\nbeta");
    session.redo();
    assert_eq!(session.raw_body(), "alpha\n\nbe ta");
}

#[test]
fn session_undo_split_restores_single_block() {
    let mut session = EditorSession::new("one two");
    session.split_active_at(4);
    assert_eq!(session.blocks().len(), 2);
    session.undo();
    assert_eq!(session.blocks().len(), 1);
    assert_eq!(session.blocks()[0].text, "one two");
}

#[test]
fn session_raw_body_serializes_lists_and_prose() {
    let session = EditorSession::new("intro\n\n- a\n- b\n\noutro");
    assert_eq!(session.blocks().len(), 3);
    assert_eq!(session.raw_body(), "intro\n\n- a\n- b\n\noutro");
}

#[test]
fn session_thematic_break_is_omitted_from_editing_blocks() {
    let session = EditorSession::new("alpha\n\n---\n\nbeta");
    assert_eq!(session.blocks().len(), 2);
    assert!(session
        .blocks()
        .iter()
        .all(|b| !matches!(b.kind, BlockKind::ThematicBreak)));
}

#[test]
fn session_multi_byte_split_lands_on_char_boundary() {
    let mut session = EditorSession::new("café latte");
    session.split_active_at(4);
    assert_eq!(session.blocks().len(), 2);
    assert_eq!(session.blocks()[0].text, "café");
    assert_eq!(session.blocks()[1].text, "latte");
}
