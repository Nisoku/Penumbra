//! Serialize a parsed [`Document`] back to canonical markdown source.

use crate::ast::{Block, BlockKind, Document, Inline, Table};

/// The separator placed between two consecutive sibling blocks.
pub fn block_separator(previous: &BlockKind, next: &BlockKind) -> &'static str {
    if matches!(previous, BlockKind::List { .. }) && matches!(next, BlockKind::List { .. }) {
        "\n"
    } else {
        "\n\n"
    }
}

/// Serialize a document to canonical markdown source.
pub fn render_markdown(doc: &Document) -> String {
    let mut out = String::new();
    serialize_blocks(&doc.blocks, &mut out);
    out
}

fn serialize_blocks(blocks: &[Block], out: &mut String) {
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push_str(block_separator(&blocks[i - 1].kind, &block.kind));
        }
        serialize_block(&block.kind, out);
    }
}

fn serialize_block(kind: &BlockKind, out: &mut String) {
    match kind {
        BlockKind::Paragraph(inlines) => serialize_inlines(inlines, out),
        BlockKind::Heading { level, children } => {
            out.push_str(&"#".repeat(*level as usize));
            out.push(' ');
            serialize_inlines(children, out);
        }
        BlockKind::CodeBlock { language, text } => {
            out.push_str("```");
            if let Some(lang) = language {
                out.push_str(lang);
            }
            out.push('\n');
            out.push_str(text.trim_end_matches('\n'));
            out.push_str("\n```");
        }
        BlockKind::List {
            ordered,
            start,
            items,
        } => {
            serialize_list(ordered, start, items, "", out);
        }
        BlockKind::Quote(children) => {
            let mut inner = String::new();
            serialize_blocks(children, &mut inner);
            for line in inner.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            if !inner.is_empty() {
                out.pop();
            }
        }
        BlockKind::ThematicBreak => out.push_str("---"),
        BlockKind::Table(table) => serialize_table(table, out),
        BlockKind::HtmlBlock(text) => {
            out.push_str(text.trim_end_matches('\n'));
        }
        BlockKind::FootnoteDefinition { name, children } => {
            out.push_str("[^");
            out.push_str(name);
            out.push_str("]: ");
            serialize_children_inline(children, out);
        }
    }
}

fn serialize_list(
    ordered: &bool,
    start: &Option<u64>,
    items: &[crate::ast::ListItem],
    indent: &str,
    out: &mut String,
) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let marker = if *ordered {
            format!("{}. ", start.unwrap_or(1) + i as u64)
        } else {
            "- ".to_owned()
        };
        out.push_str(indent);
        out.push_str(&marker);
        if let Some(checked) = item.checked {
            out.push_str(if checked { "[x] " } else { "[ ] " });
        }
        serialize_children_inline(&item.children, out);
    }
}

/// Serialize a block's children: the first child continues the current line,
/// later children are dropped onto indented continuation lines.
fn serialize_children_inline(children: &[Block], out: &mut String) {
    if let Some(first) = children.first() {
        serialize_block_inline(&first.kind, out);
    }
    for child in &children[1..] {
        out.push('\n');
        push_indented(&serialize_kind(&child.kind), "  ", out);
    }
}

/// The block's serialization, used when the block opens inside an item context.
fn serialize_block_inline(kind: &BlockKind, out: &mut String) {
    match kind {
        BlockKind::Paragraph(inlines) => serialize_inlines(inlines, out),
        BlockKind::List {
            ordered,
            start,
            items,
        } => serialize_list(ordered, start, items, "", out),
        _ => out.push_str(&serialize_kind(kind)),
    }
}

/// Serialize a block kind to a standalone string.
fn serialize_kind(kind: &BlockKind) -> String {
    let mut out = String::new();
    serialize_block(kind, &mut out);
    out
}

/// Re-emit `text` with each line prefixed by `spaces`.
fn push_indented(text: &str, spaces: &str, out: &mut String) {
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(spaces);
        out.push_str(line);
    }
}

fn serialize_table(table: &Table, out: &mut String) {
    let header = table
        .headers
        .iter()
        .map(|cells| format_cells(cells))
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str("| ");
    out.push_str(&header);
    out.push_str(" |");

    out.push('\n');
    let align_row = table
        .align
        .iter()
        .map(|a| match a {
            crate::ast::TableAlign::None => "---",
            crate::ast::TableAlign::Left => ":---",
            crate::ast::TableAlign::Center => ":---:",
            crate::ast::TableAlign::Right => "---:",
        })
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str("| ");
    out.push_str(&align_row);
    out.push_str(" |");

    for row in &table.rows {
        out.push('\n');
        let body = row
            .iter()
            .map(|cells| format_cells(cells))
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str("| ");
        out.push_str(&body);
        out.push_str(" |");
    }
}

fn format_cells(cells: &[Inline]) -> String {
    let mut cell = String::new();
    serialize_inlines(cells, &mut cell);
    cell.replace('|', "\\|")
}

fn serialize_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(text),
            Inline::Strong(children) => {
                out.push_str("**");
                serialize_inlines(children, out);
                out.push_str("**");
            }
            Inline::Emphasis(children) => {
                out.push('*');
                serialize_inlines(children, out);
                out.push('*');
            }
            Inline::Strikethrough(children) => {
                out.push_str("~~");
                serialize_inlines(children, out);
                out.push_str("~~");
            }
            Inline::Code(code) => {
                out.push('`');
                out.push_str(code);
                out.push('`');
            }
            Inline::Link {
                url,
                title,
                children,
            } => {
                out.push('[');
                serialize_inlines(children, out);
                out.push_str("](");
                out.push_str(url);
                if !title.is_empty() {
                    out.push_str(" \"");
                    out.push_str(title);
                    out.push('"');
                }
                out.push(')');
            }
            Inline::Image { url, alt, title } => {
                out.push_str("![");
                out.push_str(alt);
                out.push_str("](");
                out.push_str(url);
                if !title.is_empty() {
                    out.push_str(" \"");
                    out.push_str(title);
                    out.push('"');
                }
                out.push(')');
            }
            Inline::NoteEmbed { note_ref } => {
                out.push_str("[[");
                out.push_str(note_ref);
                out.push_str("]]");
            }
            Inline::TagRef { name } => {
                out.push('#');
                out.push_str(name);
            }
            Inline::LineBreak => out.push_str("  \n"),
            Inline::SoftBreak => out.push('\n'),
        }
    }
}
