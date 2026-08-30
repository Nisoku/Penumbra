//! The block editing session for one note body.

use edita_core::{process_nodes, Block as EditaBlock, Command as EditaCommand, Editor};
use penumbra_markdown::ast::{BlockKind, Table};
use penumbra_markdown::render::markdown::block_separator;

use crate::doc::Document;

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

/// The mutable editing state the session's commands operate on.
#[derive(Debug, Clone)]
pub struct SessionState {
    blocks: Vec<BlockEdit>,
    active: usize,
    undo: Vec<(Vec<BlockEdit>, usize)>,
    redo: Vec<(Vec<BlockEdit>, usize)>,
}

impl SessionState {
    fn empty() -> Self {
        Self {
            blocks: Vec::new(),
            active: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

/// An input the editor's block dispatch parses into a [`BlockEdit`].
struct MarkdownInput {
    kind: BlockKind,
    raw: String,
}

/// A full editing session over one note body.
pub struct EditorSession {
    inner: Editor<BlockEdit, SessionState, MarkdownInput>,
}

impl EditorSession {
    /// Parse a markdown body into an editable block session.
    pub fn new(source: &str) -> Self {
        let doc = Document::new(source);
        let inputs: Vec<MarkdownInput> = doc
            .blocks()
            .iter()
            .filter_map(|b| block_input(&doc, b))
            .collect();

        let mut inner = Editor::new(SessionState::empty());
        for kind in kind_catalog() {
            inner.add_block(KindBlock { kind });
        }
        inner.set_fallback_block(Box::new(ProseFallback));
        let blocks = process_nodes(&inner, inputs);
        *inner = SessionState {
            blocks,
            ..SessionState::empty()
        };

        Self { inner }
    }

    /// The blocks in document order.
    pub fn blocks(&self) -> &[BlockEdit] {
        &self.state().blocks
    }

    /// The index of the block being edited.
    pub fn active(&self) -> usize {
        self.state().active
    }

    /// Set which block is active, clamped to bounds.
    pub fn set_active(&mut self, index: usize) {
        let upper = self.state().blocks.len().saturating_sub(1);
        self.state_mut().active = index.min(upper);
    }

    /// The editing mode of the active block.
    pub fn active_mode(&self) -> BlockMode {
        self.state()
            .blocks
            .get(self.state().active)
            .map(|b| mode_of_kind(&b.kind))
            .unwrap_or(BlockMode::Prose)
    }

    /// Whether the active block is a code fence.
    pub fn active_is_code(&self) -> bool {
        matches!(self.active_kind(), BlockKind::CodeBlock { .. })
    }

    fn active_kind(&self) -> &BlockKind {
        &self.state().blocks[self.state().active].kind
    }

    /// True when the session holds no blocks at all.
    pub fn is_empty(&self) -> bool {
        self.state().blocks.is_empty()
    }

    /// Commit the text currently shown in the active editor.
    pub fn apply_active_text(&mut self, text: &str) {
        self.inner.command(ApplyActiveText {
            text: text.to_owned(),
        });
    }

    /// Split the active block at the given UTF-8 byte offset into two blocks.
    pub fn split_active_at(&mut self, byte_offset: usize) {
        self.inner.command(SplitActive {
            offset: byte_offset,
        });
    }

    /// Remove the active block, clamping the active index after it.
    pub fn remove_active(&mut self) {
        self.inner.command(RemoveActive);
    }

    /// Merge the active block into the previous one and make it active.
    pub fn merge_into_previous(&mut self) {
        self.inner.command(MergeIntoPrevious);
    }

    /// Undo the last edit, returning the active index it landed on.
    pub fn undo(&mut self) -> Option<usize> {
        if self.state().undo.is_empty() {
            return None;
        }
        self.inner.command(Undo);
        Some(self.state().active)
    }

    /// Redo the last undone edit, returning the active index it landed on.
    pub fn redo(&mut self) -> Option<usize> {
        if self.state().redo.is_empty() {
            return None;
        }
        self.inner.command(Redo);
        Some(self.state().active)
    }

    pub fn can_undo(&self) -> bool {
        !self.state().undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.state().redo.is_empty()
    }

    /// The full markdown body this session serializes to.
    pub fn raw_body(&self) -> String {
        let mut out = String::new();
        let blocks = &self.state().blocks;
        for (i, block) in blocks.iter().enumerate() {
            if i > 0 {
                out.push_str(block_separator(&blocks[i - 1].kind, &block.kind));
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
        match self.state().blocks.get(index).map(|b| &b.kind) {
            Some(BlockKind::Heading { level, .. }) => *level,
            _ => 0,
        }
    }

    fn state(&self) -> &SessionState {
        &self.inner
    }

    fn state_mut(&mut self) -> &mut SessionState {
        &mut self.inner
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

/// Snap the current block list into the undo stack.
fn snapshot(state: &mut SessionState) {
    state.undo.push((state.blocks.clone(), state.active));
    if state.undo.len() > HISTORY_LIMIT {
        state.undo.remove(0);
    }
}

/// Strip trailing blank separators from a parsed block's raw source range.
fn block_input(doc: &Document, block: &crate::doc::Block) -> Option<MarkdownInput> {
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
    Some(MarkdownInput {
        kind: block.kind.clone(),
        raw: text,
    })
}

fn kind_catalog() -> Vec<BlockKind> {
    vec![
        BlockKind::Paragraph(Vec::new()),
        BlockKind::Heading {
            level: 1,
            children: Vec::new(),
        },
        BlockKind::CodeBlock {
            language: None,
            text: String::new(),
        },
        BlockKind::List {
            ordered: false,
            start: None,
            items: Vec::new(),
        },
        BlockKind::Quote(Vec::new()),
        BlockKind::ThematicBreak,
        BlockKind::Table(Table {
            headers: Vec::new(),
            rows: Vec::new(),
            align: Vec::new(),
        }),
        BlockKind::HtmlBlock(String::new()),
        BlockKind::FootnoteDefinition {
            name: String::new(),
            children: Vec::new(),
        },
    ]
}

/// True when two kinds belong to the same markdown block variant.
fn same_variant(a: &BlockKind, b: &BlockKind) -> bool {
    matches!(
        (a, b),
        (BlockKind::Paragraph(_), BlockKind::Paragraph(_))
            | (BlockKind::Heading { .. }, BlockKind::Heading { .. })
            | (BlockKind::CodeBlock { .. }, BlockKind::CodeBlock { .. })
            | (BlockKind::List { .. }, BlockKind::List { .. })
            | (BlockKind::Quote(_), BlockKind::Quote(_))
            | (BlockKind::ThematicBreak, BlockKind::ThematicBreak)
            | (BlockKind::Table(_), BlockKind::Table(_))
            | (BlockKind::HtmlBlock(_), BlockKind::HtmlBlock(_))
            | (
                BlockKind::FootnoteDefinition { .. },
                BlockKind::FootnoteDefinition { .. }
            )
    )
}

/// A dispatch block owning one markdown kind's parsing and normalization.
struct KindBlock {
    kind: BlockKind,
}

impl EditaBlock for KindBlock {
    type Input = MarkdownInput;
    type Node = BlockEdit;
    type State = SessionState;

    fn accepts(&self, input: &MarkdownInput) -> bool {
        same_variant(&self.kind, &input.kind)
    }

    fn parse(
        &self,
        _editor: &Editor<BlockEdit, SessionState, MarkdownInput>,
        input: &MarkdownInput,
    ) -> BlockEdit {
        BlockEdit {
            kind: input.kind.clone(),
            text: normalize_for_edit(&input.kind, &input.raw),
        }
    }
}

/// Belts-and-suspenders block: accepts anything and stores it as prose.
struct ProseFallback;

impl EditaBlock for ProseFallback {
    type Input = MarkdownInput;
    type Node = BlockEdit;
    type State = SessionState;

    fn accepts(&self, _input: &MarkdownInput) -> bool {
        true
    }

    fn parse(
        &self,
        _editor: &Editor<BlockEdit, SessionState, MarkdownInput>,
        input: &MarkdownInput,
    ) -> BlockEdit {
        BlockEdit {
            kind: BlockKind::Paragraph(Vec::new()),
            text: normalize_for_edit(&BlockKind::Paragraph(Vec::new()), &input.raw),
        }
    }
}

// Commands

/// Apply the active block's new text.
struct ApplyActiveText {
    text: String,
}

impl EditaCommand<SessionState> for ApplyActiveText {
    fn execute(&self, state: &mut SessionState) {
        if state.blocks.is_empty() {
            return;
        }
        let index = state.active;
        let normalized = normalize_for_edit(&state.blocks[index].kind, &self.text);
        if state.blocks[index].text == normalized {
            return;
        }
        snapshot(state);
        state.blocks[index].text = normalized;
        state.redo.clear();
    }
}

/// Split the active block at a byte offset.
struct SplitActive {
    offset: usize,
}

impl EditaCommand<SessionState> for SplitActive {
    fn execute(&self, state: &mut SessionState) {
        if state.blocks.is_empty() {
            return;
        }
        let index = state.active;
        if state.blocks[index].text.trim().is_empty() {
            RemoveActive.execute(state);
            return;
        }
        if self.offset == 0 {
            snapshot(state);
            state.blocks.insert(
                index,
                BlockEdit {
                    kind: BlockKind::Paragraph(Vec::new()),
                    text: String::new(),
                },
            );
            state.active = index;
            state.redo.clear();
            return;
        }
        let source = state.blocks[index].text.clone();
        let offset = source
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= self.offset)
            .unwrap_or(source.len());
        let (head, tail) = source.split_at(offset);
        let kind = state.blocks[index].kind.clone();
        let tail_kind = match &kind {
            BlockKind::Heading { .. } | BlockKind::Table(_) => BlockKind::Paragraph(Vec::new()),
            _ => kind.clone(),
        };
        let left_text = head.trim_end().to_owned();
        let right_text = tail.trim_start().to_owned();
        if left_text.is_empty() && right_text.is_empty() {
            return;
        }
        snapshot(state);
        state.blocks[index].text = left_text;
        state.blocks.insert(
            index + 1,
            BlockEdit {
                kind: tail_kind,
                text: right_text,
            },
        );
        state.active = index + 1;
        state.redo.clear();
    }
}

/// Remove the active block, clamping the active index after it.
struct RemoveActive;

impl EditaCommand<SessionState> for RemoveActive {
    fn execute(&self, state: &mut SessionState) {
        if state.blocks.is_empty() {
            return;
        }
        snapshot(state);
        state.blocks.remove(state.active);
        state.active = if state.blocks.is_empty() {
            0
        } else {
            state.active.min(state.blocks.len() - 1)
        };
        state.redo.clear();
    }
}

/// Merge the active block into the previous one and make it active.
struct MergeIntoPrevious;

impl EditaCommand<SessionState> for MergeIntoPrevious {
    fn execute(&self, state: &mut SessionState) {
        if state.active == 0 || state.blocks.is_empty() {
            return;
        }
        let index = state.active;
        let dragged = state.blocks.remove(index);
        let both_prose = mode_of_kind(&state.blocks[index - 1].kind) == BlockMode::Prose
            && mode_of_kind(&dragged.kind) == BlockMode::Prose;
        let separator = if both_prose {
            if state.blocks[index - 1].text.is_empty()
                || dragged.text.trim().is_empty()
                || state.blocks[index - 1]
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
        snapshot(state);
        state.blocks[index - 1].text.push_str(separator);
        state.blocks[index - 1]
            .text
            .push_str(dragged.text.trim_start());
        state.active = index - 1;
        state.redo.clear();
    }
}

/// Undo the last edit.
struct Undo;

impl EditaCommand<SessionState> for Undo {
    fn execute(&self, state: &mut SessionState) {
        if let Some((blocks, active)) = state.undo.pop() {
            state
                .redo
                .push((std::mem::replace(&mut state.blocks, blocks), state.active));
            state.active = active;
        }
    }
}

/// Redo the last undone edit.
struct Redo;

impl EditaCommand<SessionState> for Redo {
    fn execute(&self, state: &mut SessionState) {
        if let Some((blocks, active)) = state.redo.pop() {
            state
                .undo
                .push((std::mem::replace(&mut state.blocks, blocks), state.active));
            state.active = active;
        }
    }
}
