use pulldown_cmark::Event;

use super::scanner::split_text_for_custom;
use super::SourceRange;
use crate::ast::{Block, BlockId, BlockKind, Inline, ListItem, TableAlign};
use penumbra_core::error::Result;

pub(super) struct Ctx {
    pub(super) blocks: Vec<Block>,
    pub(super) stack: Vec<Frame>,
    /// Byte offset of each frame's opening construct, aligned with `stack`.
    pub(super) frame_starts: Vec<usize>,
    /// Top-level block source ranges, aligned with `blocks`.
    pub(super) ranges: Vec<(usize, usize)>,
    /// Opening offset of the innermost table, for its range.
    pub(super) table_start: usize,
    /// The largest construct end seen so far, a fallback for flushed blocks.
    pub(super) last_end: usize,
    pub(super) inlines: Vec<Inline>,
    pub(super) text_buf: String,
    pub(super) table_phase: TablePhase,
    pub(super) table_headers: Vec<Vec<Inline>>,
    pub(super) table_body: Vec<Vec<Vec<Inline>>>,
    pub(super) table_row_buf: Vec<Vec<Inline>>,
    pub(super) table_align: Vec<TableAlign>,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub(super) enum TablePhase {
    #[default]
    None,
    Headers,
    Body,
}

impl Ctx {
    pub(super) fn new() -> Self {
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

    pub(super) fn handle(&mut self, event: Event<'_>, range: &std::ops::Range<usize>) -> Result<()> {
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
            Html(text) => {
                // Raw html inside a CommonMark html block
                if let Some(Frame::HtmlBlock { text: html }) = self.stack.last_mut() {
                    html.push_str(&text);
                }
            }
            InlineHtml(text) => {
                self.flush_text();
                self.inlines.push(Inline::InlineHtml(text.to_string()));
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
                self.inlines.push(Inline::FootnoteRef(text.to_string()));
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn flush_text(&mut self) {
        if self.text_buf.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.text_buf);
        for part in split_text_for_custom(&text) {
            self.inlines.push(part);
        }
    }
    pub(super) fn flush(&mut self) {
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
    pub(super) fn push_block(&mut self, block: Block, start: usize, end: usize) {
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

    pub(super) fn pop_frame(&mut self) -> Option<Frame> {
        self.stack.pop()
    }

    /// The opening offset paired with the frame `pop_frame` just removed.
    pub(super) fn pop_start(&mut self) -> usize {
        self.frame_starts.pop().unwrap_or(0)
    }

    pub(super) fn finish(mut self) -> Result<(Vec<Block>, Vec<SourceRange>)> {
        self.flush_text();
        self.flush();
        Ok((self.blocks, self.ranges))
    }
}

pub(super) enum Frame {
    Paragraph,
    Heading {
        level: u8,
    },
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    HtmlBlock {
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
