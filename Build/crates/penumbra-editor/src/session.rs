use crate::doc::{BlockKind, Document};

/// The maximum number of undo snapshots kept per session.
const HISTORY_LIMIT: usize = 64;

/// One editable block in the session: its markdown kind and its raw source
/// text with any trailing blank separator lines stripped.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdit {
    pub kind: BlockKind,
    pub text: String,
}

/// A full editing session over one note body.
pub struct EditorSession {
    blocks: Vec<BlockEdit>,
    active: usize,
    undo: Vec<(Vec<BlockEdit>, usize)>,
    redo: Vec<(Vec<BlockEdit>, usize)>,
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

impl EditorSession {
    /// Parse a markdown body into an editable block session.
    pub fn new(source: &str) -> Self {
        let doc = Document::new(source);
        let blocks = doc
            .blocks()
            .iter()
            .filter_map(|b| block_edit(&doc, b))
            .collect();
        Self {
            blocks,
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
        self.active = index.min(self.blocks.len().saturating_sub(1));
    }

    /// The editing mode of the active block.
    pub fn active_mode(&self) -> BlockMode {
        self.blocks
            .get(self.active)
            .map(mode_of_block)
            .unwrap_or(BlockMode::Prose)
    }

    /// Whether the active block is a code fence.
    pub fn active_is_code(&self) -> bool {
        matches!(self.active_kind(), BlockKind::CodeBlock(_))
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
        let normalized = normalize_for_edit(&self.blocks[self.active].kind, text);
        if self.blocks[self.active].text == normalized {
            return;
        }
        self.snapshot();
        self.blocks[self.active].text = normalized;
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
                    kind: BlockKind::Paragraph,
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
            BlockKind::Heading(_) | BlockKind::Table => BlockKind::Paragraph,
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
        if self.blocks.is_empty() {
            self.active = 0;
        } else {
            self.active = self.active.min(self.blocks.len() - 1);
        }
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
        let (blocks, active) = self.undo.pop()?;
        self.redo
            .push((std::mem::replace(&mut self.blocks, blocks), self.active));
        self.active = active;
        Some(active)
    }

    /// Redo the last undone edit, returning the active index it landed on.
    pub fn redo(&mut self) -> Option<usize> {
        let (blocks, active) = self.redo.pop()?;
        self.undo
            .push((std::mem::replace(&mut self.blocks, blocks), self.active));
        self.active = active;
        Some(active)
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
                let previous = &self.blocks[i - 1];
                let same_list = matches!(previous.kind, BlockKind::List { .. })
                    && matches!(block.kind, BlockKind::List { .. });
                out.push_str(if same_list { "\n" } else { "\n\n" });
            }
            out.push_str(block.text.trim_end());
        }
        out
    }

    /// The zero-based heading level of the active block, 0 when not a heading.
    pub fn active_heading_level(&self) -> u8 {
        match self.active_kind() {
            BlockKind::Heading(level) => *level,
            _ => 0,
        }
    }

    /// The heading level of a given block, 0 when not a heading.
    pub fn heading_level(&self, index: usize) -> u8 {
        match self.blocks.get(index).map(|b| &b.kind) {
            Some(BlockKind::Heading(level)) => *level,
            _ => 0,
        }
    }

    fn snapshot(&mut self) {
        self.undo.push((self.blocks.clone(), self.active));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }
}

/// The edit mode a given block kind belongs to.
pub fn mode_of_kind(kind: &BlockKind) -> BlockMode {
    match kind {
        BlockKind::Heading(_) => BlockMode::Heading,
        BlockKind::CodeBlock(_) => BlockMode::Raw,
        _ => BlockMode::Prose,
    }
}

fn mode_of_block(block: &BlockEdit) -> BlockMode {
    mode_of_kind(&block.kind)
}

/// Normalize incoming editor text for storage in a block of this kind.
fn normalize_for_edit(kind: &BlockKind, text: &str) -> String {
    match kind {
        BlockKind::CodeBlock(_) => text.trim_end().to_owned(),
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

/// Strip trailing blank separators from a parsed block's raw source range.
fn block_edit(doc: &Document, block: &crate::doc::Block) -> Option<BlockEdit> {
    if matches!(block.kind, BlockKind::ThematicBreak) {
        return None;
    }
    let (start, end) = block.source_range;
    if start >= end {
        return None;
    }
    let text = doc.source[start..end].trim_end_matches('\n').to_owned();
    if text.is_empty() {
        return None;
    }
    Some(BlockEdit {
        kind: block.kind.clone(),
        text,
    })
}
