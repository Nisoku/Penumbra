//! The block editing session for one note body.

use penumbra_markdown::ast::BlockKind;
use penumbra_markdown::parser::parse_blocks_offsets;
use penumbra_markdown::render::markdown::block_separator;

/// The maximum number of undo snapshots kept per session.
const HISTORY_LIMIT: usize = 64;

/// One editable block in the session: its markdown kind and its raw source
/// text with any trailing blank separator lines stripped.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdit {
    pub kind: BlockKind,
    pub text: String,
}

/// Kind family that controls how an active block is rendered and edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMode {
    /// Prose: rendered as markdown, edited single-newline, Enter splits.
    Prose,
    /// Headings are single-line; Enter drops the tail into a paragraph.
    Heading,
    /// Raw source edited verbatim, Enter inserts a literal newline.
    Raw,
}

/// A full editing session over one note body.
pub struct EditorSession {
    blocks: Vec<BlockEdit>,
    active: usize,
    /// Undo journal of (serialized body, active index) snapshots.
    undo: Vec<(String, usize)>,
    /// Redo journal of (serialized body, active index) snapshots.
    redo: Vec<(String, usize)>,
}

impl EditorSession {
    /// Parse a markdown body into an editable block session.
    pub fn new(source: &str) -> Self {
        Self {
            blocks: Self::blocks_of(source),
            active: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// The blocks in document order.
    pub fn blocks(&self) -> &[BlockEdit] {
        &self.blocks
    }

    /// The index of the block being edited.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Set which block is active, clamped to bounds.
    pub fn set_active(&mut self, index: usize) {
        let upper = self.blocks.len().saturating_sub(1);
        self.active = index.min(upper);
    }

    /// The editing mode of the active block.
    pub fn active_mode(&self) -> BlockMode {
        self.blocks
            .get(self.active)
            .map(|b| mode_of_kind(&b.kind))
            .unwrap_or(BlockMode::Prose)
    }

    /// Whether the active block is a code fence.
    pub fn active_is_code(&self) -> bool {
        matches!(self.active_kind(), BlockKind::CodeBlock { .. })
    }

    fn active_kind(&self) -> &BlockKind {
        &self.blocks[self.active].kind
    }

    /// True when the session holds no blocks at all.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Commit the text currently shown in the active editor.
    pub fn apply_active_text(&mut self, text: &str) {
        if self.blocks.is_empty() {
            return;
        }
        let index = self.active;
        let normalized = normalize_for_edit(&self.blocks[index].kind, text);
        if self.blocks[index].text == normalized {
            return;
        }
        if let Some(derived) = rederive_kind(&normalized) {
            self.blocks[index].kind = derived;
        }
        self.snapshot();
        self.blocks[index].text = normalized;
        self.redo.clear();
    }

    /// Split the active block at the given UTF-8 byte offset into two blocks.
    pub fn split_active_at(&mut self, byte_offset: usize) {
        if self.blocks.is_empty() {
            return;
        }
        let index = self.active;
        if self.blocks[index].text.trim().is_empty() {
            self.remove_active();
            return;
        }
        if byte_offset == 0 {
            self.snapshot();
            self.blocks.insert(
                index,
                BlockEdit {
                    kind: BlockKind::Paragraph(Vec::new()),
                    text: String::new(),
                },
            );
            self.active = index;
            self.redo.clear();
            return;
        }
        let source = self.blocks[index].text.clone();
        let offset = source
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= byte_offset)
            .unwrap_or(source.len());
        let (head, tail) = source.split_at(offset);
        let kind = self.blocks[index].kind.clone();
        let tail_kind = match &kind {
            BlockKind::Heading { .. } | BlockKind::Table(_) => BlockKind::Paragraph(Vec::new()),
            _ => kind.clone(),
        };
        let left_text = head.trim_end().to_owned();
        let right_text = tail.trim_start().to_owned();
        if left_text.is_empty() && right_text.is_empty() {
            return;
        }
        self.snapshot();
        self.blocks[index].text = left_text;
        self.blocks.insert(
            index + 1,
            BlockEdit {
                kind: tail_kind,
                text: right_text,
            },
        );
        self.active = index + 1;
        self.redo.clear();
    }

    /// Remove the active block, clamping the active index after it.
    pub fn remove_active(&mut self) {
        if self.blocks.is_empty() {
            return;
        }
        self.snapshot();
        self.blocks.remove(self.active);
        self.active = if self.blocks.is_empty() {
            0
        } else {
            self.active.min(self.blocks.len() - 1)
        };
        self.redo.clear();
    }

    /// Merge the active block into the previous one and make it active.
    pub fn merge_into_previous(&mut self) {
        if self.active == 0 || self.blocks.is_empty() {
            return;
        }
        let index = self.active;
        let dragged = self.blocks.remove(index);
        let both_prose = mode_of_kind(&self.blocks[index - 1].kind) == BlockMode::Prose
            && mode_of_kind(&dragged.kind) == BlockMode::Prose;
        let separator = if both_prose {
            if self.blocks[index - 1].text.is_empty()
                || dragged.text.trim().is_empty()
                || self.blocks[index - 1]
                    .text
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_whitespace())
            {
                ""
            } else {
                " "
            }
        } else {
            "\n"
        };
        self.snapshot();
        self.blocks[index - 1].text.push_str(separator);
        self.blocks[index - 1]
            .text
            .push_str(dragged.text.trim_start());
        self.active = index - 1;
        self.redo.clear();
    }

    /// Undo the last edit, returning the active index it landed on.
    pub fn undo(&mut self) -> Option<usize> {
        let (body, active) = self.undo.pop()?;
        self.redo.push((self.raw_body(), self.active));
        self.restore(&body, active);
        Some(self.active)
    }

    /// Redo the last undone edit, returning the active index it landed on.
    pub fn redo(&mut self) -> Option<usize> {
        let (body, active) = self.redo.pop()?;
        self.undo.push((self.raw_body(), self.active));
        self.restore(&body, active);
        Some(self.active)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The full markdown body this session serializes to.
    pub fn raw_body(&self) -> String {
        let mut out = String::new();
        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                out.push_str(block_separator(&self.blocks[i - 1].kind, &block.kind));
            }
            out.push_str(block.text.trim_end());
        }
        out
    }

    /// The zero-based heading level of the active block, 0 when not a heading.
    pub fn active_heading_level(&self) -> u8 {
        match self.active_kind() {
            BlockKind::Heading { level, .. } => *level,
            _ => 0,
        }
    }

    /// The heading level of a given block, 0 when not a heading.
    pub fn heading_level(&self, index: usize) -> u8 {
        match self.blocks.get(index).map(|b| &b.kind) {
            Some(BlockKind::Heading { level, .. }) => *level,
            _ => 0,
        }
    }

    /// Derive the editable block list from a markdown bodyy
    fn blocks_of(source: &str) -> Vec<BlockEdit> {
        let Ok(pairs) = parse_blocks_offsets(source) else {
            return Vec::new();
        };
        pairs
            .into_iter()
            .filter_map(|(block, (start, end))| {
                if start >= end {
                    return None;
                }
                let text = source[start..end].trim_end_matches('\n').to_owned();
                if text.is_empty() {
                    return None;
                }
                Some(BlockEdit {
                    kind: block.kind,
                    text,
                })
            })
            .collect()
    }

    /// Snap the current state into the undo stack.
    fn snapshot(&mut self) {
        self.undo.push((self.raw_body(), self.active));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }

    /// Replace the block list with the re-parse of a journaled body.
    fn restore(&mut self, body: &str, active: usize) {
        self.blocks = Self::blocks_of(body);
        self.active = active.min(self.blocks.len().saturating_sub(1));
    }
}

/// The edit mode a given block kind belongs to.
pub fn mode_of_kind(kind: &BlockKind) -> BlockMode {
    match kind {
        BlockKind::Heading { .. } => BlockMode::Heading,
        BlockKind::CodeBlock { .. } => BlockMode::Raw,
        _ => BlockMode::Prose,
    }
}

/// Normalize incoming editor text for storage in a block of this kind.
fn normalize_for_edit(kind: &BlockKind, text: &str) -> String {
    match kind {
        BlockKind::CodeBlock { .. } => text.trim_end().to_owned(),
        _ => {
            let mut normalized = String::new();
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if !normalized.is_empty() {
                    normalized.push('\n');
                }
                normalized.push_str(line);
            }
            normalized
        }
    }
}

/// Re-derive a block's kind from the markdown its text parses as cause we need to sometimes
fn rederive_kind(text: &str) -> Option<BlockKind> {
    let pairs = parse_blocks_offsets(text).ok()?;
    if pairs.len() == 1 {
        pairs.first().map(|(block, _)| block.kind.clone())
    } else {
        None
    }
}
