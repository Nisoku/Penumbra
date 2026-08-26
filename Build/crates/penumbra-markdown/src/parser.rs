use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

use crate::ast::{Block, BlockId, BlockKind, Document, Inline, ListItem, Table, TableAlign};
use penumbra_core::error::Result;

pub fn parse_document(text: &str) -> Result<Document> {
    let blocks = run_parser(text)?;
    Ok(Document { blocks })
}

pub fn parse_block(text: &str) -> Result<Vec<Block>> {
    run_parser(text)
}

fn run_parser(text: &str) -> Result<Vec<Block>> {
    let options = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_FOOTNOTES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS
        | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = pulldown_cmark::Parser::new_ext(text, options);
    let mut ctx = Ctx::new();
    for event in parser {
        ctx.handle(event)?;
    }
    ctx.finish()
}

struct Ctx {
    blocks: Vec<Block>,
    stack: Vec<Frame>,
    inlines: Vec<Inline>,
    text_buf: String,
    table_phase: TablePhase,
    table_headers: Vec<Vec<Inline>>,
    table_body: Vec<Vec<Vec<Inline>>>,
    table_row_buf: Vec<Vec<Inline>>,
    table_align: Vec<TableAlign>,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum TablePhase {
    #[default]
    None,
    Headers,
    Body,
}

impl Ctx {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            stack: Vec::new(),
            inlines: Vec::new(),
            text_buf: String::new(),
            table_phase: TablePhase::None,
            table_headers: Vec::new(),
            table_body: Vec::new(),
            table_row_buf: Vec::new(),
            table_align: Vec::new(),
        }
    }

    fn handle(&mut self, event: Event<'_>) -> Result<()> {
        use Event::*;
        match event {
            Start(tag) => {
                self.flush_text();
                self.handle_start(tag);
            }
            End(tag) => {
                self.flush_text();
                self.handle_end(tag)?;
            }
            Text(text) => {
                let in_code_block = self
                    .stack
                    .last()
                    .is_some_and(|f| matches!(f, Frame::CodeBlock { .. }));
                if in_code_block {
                    if let Some(Frame::CodeBlock {
                        text: code_text, ..
                    }) = self.stack.last_mut()
                    {
                        code_text.push_str(&text);
                    }
                } else {
                    self.text_buf.push_str(&text);
                }
            }
            Code(text) => {
                self.flush_text();
                self.inlines.push(Inline::Code(text.to_string()));
            }
            Html(text) | InlineHtml(text) => {
                self.flush_text();
                self.inlines.push(Inline::Text(text.to_string()));
            }
            SoftBreak => {
                self.flush_text();
                self.inlines.push(Inline::SoftBreak);
            }
            HardBreak => {
                self.flush_text();
                self.inlines.push(Inline::LineBreak);
            }
            Rule => {
                self.flush_text();
                self.flush();
                self.blocks.push(Block {
                    id: BlockId::new(),
                    kind: BlockKind::ThematicBreak,
                });
            }
            TaskListMarker(checked) => {
                self.flush_text();
                if let Some(Frame::ListItem {
                    ref mut checked_mark,
                    ..
                }) = self.stack.last_mut()
                {
                    *checked_mark = Some(checked);
                }
            }
            FootnoteReference(text) => {
                self.flush_text();
                self.inlines.push(Inline::Text(format!("[^{}]", text)));
            }
            _ => {}
        }
        Ok(())
    }

    fn flush_text(&mut self) {
        if self.text_buf.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.text_buf);
        for part in split_text_for_custom(&text) {
            self.inlines.push(part);
        }
    }

    fn handle_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush();
                self.stack.push(Frame::Paragraph);
            }
            Tag::Heading { level, .. } => {
                self.flush();
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.stack.push(Frame::Heading { level: lvl });
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => {
                        let s = l.to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.stack.push(Frame::CodeBlock {
                    language: lang,
                    text: String::new(),
                });
            }
            Tag::List(start) => {
                self.flush();
                self.stack.push(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.stack.push(Frame::ListItem {
                    checked_mark: None,
                    children: Vec::new(),
                });
            }
            Tag::Table(alignments) => {
                self.flush();
                self.table_phase = TablePhase::Headers;
                self.table_headers.clear();
                self.table_body.clear();
                self.table_row_buf.clear();
                self.table_align = alignments
                    .into_iter()
                    .map(|a| match a {
                        Alignment::Left => TableAlign::Left,
                        Alignment::Center => TableAlign::Center,
                        Alignment::Right => TableAlign::Right,
                        Alignment::None => TableAlign::None,
                    })
                    .collect();
            }
            Tag::TableHead => {
                self.table_phase = TablePhase::Headers;
            }
            Tag::TableRow => {}
            Tag::TableCell => {}
            Tag::FootnoteDefinition(name) => {
                self.flush();
                self.stack.push(Frame::FootnoteDefinition {
                    name: name.to_string(),
                    children: Vec::new(),
                });
            }
            Tag::Emphasis => {
                self.stack.push(Frame::Emphasis {
                    saved_len: self.inlines.len(),
                });
            }
            Tag::Strong => {
                self.stack.push(Frame::Strong {
                    saved_len: self.inlines.len(),
                });
            }
            Tag::Strikethrough => {
                self.stack.push(Frame::Strikethrough {
                    saved_len: self.inlines.len(),
                });
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Link {
                    url: dest_url.to_string(),
                    title: title.to_string(),
                    saved_len: self.inlines.len(),
                });
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Image {
                    url: dest_url.to_string(),
                    title: title.to_string(),
                });
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd) -> Result<()> {
        match tag {
            TagEnd::Paragraph => {
                self.pop_frame();
                let children = std::mem::take(&mut self.inlines);
                self.push_block(Block {
                    id: BlockId::new(),
                    kind: BlockKind::Paragraph(children),
                });
            }
            TagEnd::Heading(_) => {
                if let Some(Frame::Heading { level }) = self.pop_frame() {
                    let children = std::mem::take(&mut self.inlines);
                    self.push_block(Block {
                        id: BlockId::new(),
                        kind: BlockKind::Heading { level, children },
                    });
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::Quote(children)) = self.pop_frame() {
                    self.push_block(Block {
                        id: BlockId::new(),
                        kind: BlockKind::Quote(children),
                    });
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Frame::CodeBlock { language, text }) = self.pop_frame() {
                    self.push_block(Block {
                        id: BlockId::new(),
                        kind: BlockKind::CodeBlock { language, text },
                    });
                }
            }
            TagEnd::List(ordered) => {
                if let Some(Frame::List { items, start, .. }) = self.pop_frame() {
                    self.push_block(Block {
                        id: BlockId::new(),
                        kind: BlockKind::List {
                            ordered,
                            start,
                            items,
                        },
                    });
                }
            }
            TagEnd::Item => {
                // Tight list items have no Paragraph wrapper, so flush
                // any accumulated inlines as a paragraph into the item.
                if !self.inlines.is_empty() {
                    let inlines = std::mem::take(&mut self.inlines);
                    if let Some(Frame::ListItem {
                        ref mut children, ..
                    }) = self.stack.last_mut()
                    {
                        children.push(Block {
                            id: BlockId::new(),
                            kind: BlockKind::Paragraph(inlines),
                        });
                    }
                }
                if let Some(Frame::ListItem {
                    checked_mark,
                    children,
                }) = self.pop_frame()
                {
                    if let Some(Frame::List { ref mut items, .. }) = self.stack.last_mut() {
                        items.push(ListItem {
                            checked: checked_mark,
                            children,
                        });
                    }
                }
            }
            TagEnd::Table => {
                let row = std::mem::take(&mut self.table_row_buf);
                if !row.is_empty() {
                    match self.table_phase {
                        TablePhase::Headers => self.table_headers = row,
                        TablePhase::Body => self.table_body.push(row),
                        TablePhase::None => {}
                    }
                }
                let align = std::mem::take(&mut self.table_align);
                let headers = std::mem::take(&mut self.table_headers);
                let rows = std::mem::take(&mut self.table_body);
                self.table_phase = TablePhase::None;
                self.push_block(Block {
                    id: BlockId::new(),
                    kind: BlockKind::Table(Table {
                        headers,
                        rows,
                        align,
                    }),
                });
            }
            TagEnd::TableHead => {
                let row = std::mem::take(&mut self.table_row_buf);
                if !row.is_empty() {
                    self.table_headers = row;
                }
                self.table_phase = TablePhase::Body;
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.inlines);
                self.table_row_buf.push(cell);
            }
            TagEnd::TableRow => {
                let row = std::mem::take(&mut self.table_row_buf);
                if !row.is_empty() {
                    match self.table_phase {
                        TablePhase::Headers => self.table_headers = row,
                        TablePhase::Body => self.table_body.push(row),
                        TablePhase::None => {}
                    }
                }
            }
            TagEnd::FootnoteDefinition => {
                if let Some(Frame::FootnoteDefinition { name, children }) = self.pop_frame() {
                    self.push_block(Block {
                        id: BlockId::new(),
                        kind: BlockKind::FootnoteDefinition { name, children },
                    });
                }
            }
            TagEnd::Emphasis => {
                if let Some(Frame::Emphasis { saved_len }) = self.pop_frame() {
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Emphasis(children));
                }
            }
            TagEnd::Strong => {
                if let Some(Frame::Strong { saved_len }) = self.pop_frame() {
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Strong(children));
                }
            }
            TagEnd::Strikethrough => {
                if let Some(Frame::Strikethrough { saved_len }) = self.pop_frame() {
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Strikethrough(children));
                }
            }
            TagEnd::Link => {
                if let Some(Frame::Link {
                    url,
                    title,
                    saved_len,
                }) = self.pop_frame()
                {
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Link {
                        url,
                        title,
                        children,
                    });
                }
            }
            TagEnd::Image => {
                if let Some(Frame::Image { url, title }) = self.pop_frame() {
                    let alt_inlines = std::mem::take(&mut self.inlines);
                    let mut alt_text = String::new();
                    for child in &alt_inlines {
                        child.write_plain_text(&mut alt_text);
                    }
                    self.inlines.push(Inline::Image {
                        url,
                        alt: alt_text,
                        title,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn flush(&mut self) {
        if self.inlines.is_empty() {
            return;
        }
        let children = std::mem::take(&mut self.inlines);
        self.push_block(Block {
            id: BlockId::new(),
            kind: BlockKind::Paragraph(children),
        });
    }

    fn push_block(&mut self, block: Block) {
        for frame in self.stack.iter_mut().rev() {
            match frame {
                Frame::Quote(ref mut children) => {
                    children.push(block);
                    return;
                }
                Frame::ListItem {
                    ref mut children, ..
                } => {
                    children.push(block);
                    return;
                }
                Frame::FootnoteDefinition {
                    ref mut children, ..
                } => {
                    children.push(block);
                    return;
                }
                _ => {}
            }
        }
        self.blocks.push(block);
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        self.stack.pop()
    }

    fn finish(mut self) -> Result<Vec<Block>> {
        self.flush_text();
        self.flush();
        Ok(self.blocks)
    }
}

enum Frame {
    Paragraph,
    Heading {
        level: u8,
    },
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    ListItem {
        checked_mark: Option<bool>,
        children: Vec<Block>,
    },
    Quote(Vec<Block>),
    FootnoteDefinition {
        name: String,
        children: Vec<Block>,
    },
    Emphasis {
        saved_len: usize,
    },
    Strong {
        saved_len: usize,
    },
    Strikethrough {
        saved_len: usize,
    },
    Link {
        url: String,
        title: String,
        saved_len: usize,
    },
    Image {
        url: String,
        title: String,
    },
}

fn split_text_for_custom(text: &str) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::new();
    let mut buf = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'[' && bytes.get(i + 1) == Some(&b'[') {
            if let Some(end) = find_closing_bracket(text, i + 2) {
                let ref_text = &text[i + 2..end];
                if !ref_text.is_empty() {
                    if !buf.is_empty() {
                        result.push(Inline::Text(std::mem::take(&mut buf)));
                    }
                    result.push(Inline::NoteEmbed {
                        note_ref: ref_text.to_string(),
                    });
                    i = end + 2;
                    continue;
                }
            }
        }

        if bytes[i] == b'#'
            && (i == 0 || is_tag_boundary(bytes[i - 1]))
            && text[i + 1..]
                .chars()
                .next()
                .is_some_and(|next| !next.is_whitespace() && next != '#')
        {
            let tag_end = find_tag_end(text, i + 1);
            if tag_end > i + 1 {
                if !buf.is_empty() {
                    result.push(Inline::Text(std::mem::take(&mut buf)));
                }
                result.push(Inline::TagRef {
                    name: text[i + 1..tag_end].to_string(),
                });
                i = tag_end;
                continue;
            }
        }

        let ch = text[i..].chars().next().expect("i is a char boundary");
        buf.push(ch);
        i += ch.len_utf8();
    }

    if !buf.is_empty() {
        result.push(Inline::Text(buf));
    }

    result
}

fn find_closing_bracket(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b']' {
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    None
}

fn find_tag_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for c in text[start..].chars() {
        // Unicode letters keep non-ASCII tags intact;
        // slashes allow nested tags like #projects/penumbra.
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '/' {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn is_tag_boundary(b: u8) -> bool {
    b.is_ascii_whitespace()
        || b == b'('
        || b == b'['
        || b == b','
        || b == b'.'
        || b == b'!'
        || b == b'?'
        || b == b':'
        || b == b';'
}

pub fn markdown_to_html(text: &str) -> Result<String> {
    let doc = parse_document(text)?;
    Ok(crate::render::html::render_html(&doc))
}

pub fn markdown_to_plain(text: &str) -> Result<String> {
    let doc = parse_document(text)?;
    Ok(doc.plain_text())
}
