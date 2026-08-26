use std::collections::HashSet;

use penumbra_editor::{
    apply_command, BlockKind, BlockMark, Command, Cursor, Document, Journal, Selection, ViewModel,
};

// Document

#[test]
fn doc_parse_paragraph() {
    let doc = Document::new("# Hello\n\nWorld");
    assert_eq!(doc.blocks().len(), 2);
    assert!(matches!(doc.blocks()[0].kind, BlockKind::Heading(1)));
    assert!(matches!(doc.blocks()[1].kind, BlockKind::Paragraph));
}

#[test]
fn doc_parse_code_block() {
    let doc = Document::new("```\nfoo\n```");
    assert_eq!(doc.blocks().len(), 1);
    assert!(matches!(&doc.blocks()[0].kind, BlockKind::CodeBlock(_)));
}

#[test]
fn doc_parse_list() {
    let doc = Document::new("1. First\n2. Second");
    assert_eq!(doc.blocks().len(), 1);
    assert!(matches!(
        &doc.blocks()[0].kind,
        BlockKind::List { ordered: true }
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
    assert_eq!(doc.block(first_id).unwrap().kind, BlockKind::Heading(1));
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
    assert!(matches!(&vm.blocks()[0].kind, BlockKind::Heading(1)));
    assert!(matches!(&vm.blocks()[1].kind, BlockKind::Paragraph));
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
    assert!(matches!(&doc.blocks()[0].kind, BlockKind::Paragraph));
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
