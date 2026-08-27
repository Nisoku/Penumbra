use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

static NEXT_BLOCK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(u64);

impl BlockId {
    pub fn new() -> Self {
        Self(NEXT_BLOCK_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for BlockId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn empty() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn plain_text(&self) -> String {
        let mut buf = String::new();
        for block in &self.blocks {
            if !buf.is_empty() {
                buf.push('\n');
            }
            block.kind.write_plain_text(&mut buf);
        }
        buf
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockKind {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        children: Vec<Inline>,
    },
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Quote(Vec<Block>),
    ThematicBreak,
    Table(Table),
    HtmlBlock(String),
    FootnoteDefinition {
        name: String,
        children: Vec<Block>,
    },
}

impl BlockKind {
    pub fn write_plain_text(&self, buf: &mut String) {
        match self {
            BlockKind::Paragraph(children) | BlockKind::Heading { children, .. } => {
                for child in children {
                    child.write_plain_text(buf);
                }
            }
            BlockKind::CodeBlock { text, .. } => buf.push_str(text),
            BlockKind::List { items, .. } => {
                for item in items {
                    for child in &item.children {
                        child.kind.write_plain_text(buf);
                    }
                    buf.push('\n');
                }
            }
            BlockKind::Quote(children) => {
                for child in children {
                    child.kind.write_plain_text(buf);
                }
            }
            BlockKind::ThematicBreak => {}
            BlockKind::Table(table) => {
                for row in &table.rows {
                    for cell in row {
                        for inline in cell {
                            inline.write_plain_text(buf);
                        }
                        buf.push(' ');
                    }
                    buf.push('\n');
                }
            }
            BlockKind::HtmlBlock(html) => buf.push_str(&strip_html(html)),
            BlockKind::FootnoteDefinition { children, .. } => {
                for child in children {
                    child.kind.write_plain_text(buf);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub children: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub headers: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
    pub align: Vec<TableAlign>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TableAlign {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        url: String,
        title: String,
        children: Vec<Inline>,
    },
    Image {
        url: String,
        alt: String,
        title: String,
    },
    NoteEmbed {
        note_ref: String,
    },
    TagRef {
        name: String,
    },
    LineBreak,
    SoftBreak,
}

impl Inline {
    pub fn write_plain_text(&self, buf: &mut String) {
        match self {
            Inline::Text(t) | Inline::Code(t) => buf.push_str(t),
            Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Strikethrough(children)
            | Inline::Link { children, .. } => {
                for child in children {
                    child.write_plain_text(buf);
                }
            }
            Inline::Image { alt, .. } => buf.push_str(alt),
            Inline::NoteEmbed { note_ref } => buf.push_str(note_ref),
            Inline::TagRef { name } => buf.push_str(name),
            Inline::LineBreak | Inline::SoftBreak => buf.push(' '),
        }
    }
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
