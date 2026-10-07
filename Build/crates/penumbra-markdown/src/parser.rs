use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

use crate::ast::{Block, BlockId, BlockKind, Document, Inline, ListItem, Table, TableAlign};
use penumbra_core::error::Result;

pub fn parse_document(text: &str) -> Result<Document> {
    let (blocks, _) = run_offsets(text)?;
    Ok(Document { blocks })
}

pub fn parse_block(text: &str) -> Result<Vec<Block>> {
    Ok(run_offsets(text)?.0)
}

/// Parse into top-level blocks paired with their byte range in the source
/// A half-open byte range of a block in the parsed source text.
pub type SourceRange = (usize, usize);

/// Parse into top-level blocks paired with their byte range in the source
pub fn parse_blocks_offsets(text: &str) -> Result<Vec<(Block, SourceRange)>> {
    let (blocks, ranges) = run_offsets(text)?;
    debug_assert_eq!(blocks.len(), ranges.len());
    Ok(blocks.into_iter().zip(ranges).collect())
}

fn run_offsets(text: &str) -> Result<(Vec<Block>, Vec<SourceRange>)> {
    let options = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_FOOTNOTES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS
        | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = pulldown_cmark::Parser::new_ext(text, options);
    let mut ctx = Ctx::new();
    for (event, range) in parser.into_offset_iter() {
        ctx.handle(event, &range)?;
        ctx.last_end = ctx.last_end.max(range.end);
    }
    ctx.finish()
}

struct Ctx {
    blocks: Vec<Block>,
    stack: Vec<Frame>,
    /// Byte offset of each frame's opening construct, aligned with `stack`.
    frame_starts: Vec<usize>,
    /// Top-level block source ranges, aligned with `blocks`.
    ranges: Vec<(usize, usize)>,
    /// Opening offset of the innermost table, for its range.
    table_start: usize,
    /// The largest construct end seen so far, a fallback for flushed blocks.
    last_end: usize,
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
            frame_starts: Vec::new(),
            ranges: Vec::new(),
            table_start: 0,
            last_end: 0,
            inlines: Vec::new(),
            text_buf: String::new(),
            table_phase: TablePhase::None,
            table_headers: Vec::new(),
            table_body: Vec::new(),
            table_row_buf: Vec::new(),
            table_align: Vec::new(),
        }
    }

    fn handle(&mut self, event: Event<'_>, range: &std::ops::Range<usize>) -> Result<()> {
        use Event::*;
        match event {
            Start(tag) => {
                self.flush_text();
                self.handle_start(tag, range.start);
            }
            End(tag) => {
                self.flush_text();
                self.handle_end(tag, range.end)?;
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
                self.push_block(
                    Block {
                        id: BlockId::new(),
                        kind: BlockKind::ThematicBreak,
                    },
                    range.start,
                    range.end,
                );
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

    fn handle_start(&mut self, tag: Tag<'_>, start: usize) {
        match tag {
            Tag::Paragraph => {
                self.flush();
                self.stack.push(Frame::Paragraph);
                self.frame_starts.push(start);
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
                self.frame_starts.push(start);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.stack.push(Frame::Quote(Vec::new()));
                self.frame_starts.push(start);
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
                self.frame_starts.push(start);
            }
            Tag::List(start_no) => {
                self.flush();
                self.stack.push(Frame::List {
                    start: start_no,
                    items: Vec::new(),
                });
                self.frame_starts.push(start);
            }
            Tag::Item => {
                self.stack.push(Frame::ListItem {
                    checked_mark: None,
                    children: Vec::new(),
                });
                self.frame_starts.push(start);
            }
            Tag::Table(alignments) => {
                self.flush();
                self.table_start = start;
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
                self.frame_starts.push(start);
            }
            Tag::Emphasis => {
                self.stack.push(Frame::Emphasis {
                    saved_len: self.inlines.len(),
                });
                self.frame_starts.push(start);
            }
            Tag::Strong => {
                self.stack.push(Frame::Strong {
                    saved_len: self.inlines.len(),
                });
                self.frame_starts.push(start);
            }
            Tag::Strikethrough => {
                self.stack.push(Frame::Strikethrough {
                    saved_len: self.inlines.len(),
                });
                self.frame_starts.push(start);
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Link {
                    url: dest_url.to_string(),
                    title: title.to_string(),
                    saved_len: self.inlines.len(),
                });
                self.frame_starts.push(start);
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Image {
                    url: dest_url.to_string(),
                    title: title.to_string(),
                });
                self.frame_starts.push(start);
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd, end: usize) -> Result<()> {
        match tag {
            TagEnd::Paragraph => {
                self.pop_frame();
                let start = self.pop_start();
                let children = std::mem::take(&mut self.inlines);
                self.push_block(
                    Block {
                        id: BlockId::new(),
                        kind: BlockKind::Paragraph(children),
                    },
                    start,
                    end,
                );
            }
            TagEnd::Heading(_) => {
                if let Some(Frame::Heading { level }) = self.pop_frame() {
                    let start = self.pop_start();
                    let children = std::mem::take(&mut self.inlines);
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::Heading { level, children },
                        },
                        start,
                        end,
                    );
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::Quote(children)) = self.pop_frame() {
                    let start = self.pop_start();
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::Quote(children),
                        },
                        start,
                        end,
                    );
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Frame::CodeBlock { language, text }) = self.pop_frame() {
                    let start = self.pop_start();
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::CodeBlock { language, text },
                        },
                        start,
                        end,
                    );
                }
            }
            TagEnd::List(ordered) => {
                if let Some(Frame::List { items, start, .. }) = self.pop_frame() {
                    let block_start = self.pop_start();
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::List {
                                ordered,
                                start,
                                items,
                            },
                        },
                        block_start,
                        end,
                    );
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
                    let _ = self.pop_start();
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
                self.push_block(
                    Block {
                        id: BlockId::new(),
                        kind: BlockKind::Table(Table {
                            headers,
                            rows,
                            align,
                        }),
                    },
                    self.table_start,
                    end,
                );
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
                    let start = self.pop_start();
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::FootnoteDefinition { name, children },
                        },
                        start,
                        end,
                    );
                }
            }
            TagEnd::Emphasis => {
                if let Some(Frame::Emphasis { saved_len }) = self.pop_frame() {
                    let _ = self.pop_start();
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Emphasis(children));
                }
            }
            TagEnd::Strong => {
                if let Some(Frame::Strong { saved_len }) = self.pop_frame() {
                    let _ = self.pop_start();
                    let children: Vec<Inline> = self.inlines.drain(saved_len..).collect();
                    self.inlines.push(Inline::Strong(children));
                }
            }
            TagEnd::Strikethrough => {
                if let Some(Frame::Strikethrough { saved_len }) = self.pop_frame() {
                    let _ = self.pop_start();
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
                    let _ = self.pop_start();
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
                    let _ = self.pop_start();
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
        self.push_block(
            Block {
                id: BlockId::new(),
                kind: BlockKind::Paragraph(children),
            },
            self.last_end,
            self.last_end,
        );
    }

    /// Push a block, recording its source range only when it lands at the
    /// top level (nested blocks belong to their container's range instead).
    fn push_block(&mut self, block: Block, start: usize, end: usize) {
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
        self.ranges.push((start, end));
        self.blocks.push(block);
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        self.stack.pop()
    }

    /// The opening offset paired with the frame `pop_frame` just removed.
    fn pop_start(&mut self) -> usize {
        self.frame_starts.pop().unwrap_or(0)
    }

    fn finish(mut self) -> Result<(Vec<Block>, Vec<SourceRange>)> {
        self.flush_text();
        self.flush();
        Ok((self.blocks, self.ranges))
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
