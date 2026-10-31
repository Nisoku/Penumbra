use pulldown_cmark::{Alignment, CodeBlockKind, HeadingLevel, Tag, TagEnd};

use super::builder::{Ctx, Frame, TablePhase};
use crate::ast::{Block, BlockId, BlockKind, Inline, ListItem, Table, TableAlign};
use penumbra_core::error::Result;

impl Ctx {
    pub(super) fn handle_start(&mut self, tag: Tag<'_>, start: usize) {
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
            Tag::HtmlBlock => {
                self.flush();
                self.stack.push(Frame::HtmlBlock {
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

    pub(super) fn handle_end(&mut self, tag: TagEnd, end: usize) -> Result<()> {
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
            TagEnd::HtmlBlock => {
                if let Some(Frame::HtmlBlock { text }) = self.pop_frame() {
                    let start = self.pop_start();
                    self.push_block(
                        Block {
                            id: BlockId::new(),
                            kind: BlockKind::HtmlBlock(text),
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
}
