//! A COSMIC Text-based rich text editor rendered into a Slint surface.

mod engine;
mod format;
mod runs;

use engine::EditorState;
use penumbra_markdown::ast::{Block, BlockId, BlockKind, Document, Inline};
use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::markdown::render_markdown;
use slint::Image;
use std::{cell::RefCell, rc::Rc};

const WIDTH: u32 = 760;
const HEIGHT: u32 = 440;
const CONTENT_PADDING_X: i32 = 28;
const CONTENT_PADDING_TOP: i32 = 24;
const CONTENT_PADDING_BOTTOM: i32 = 28;

/// A handle to the rich editor.
///
/// Creates an `Image` surface via [`RichEditor::surface`] that Slint displays,
/// and accepts input events through [`RichEditor::input_key`],
/// [`RichEditor::pointer_down`], [`RichEditor::pointer_drag`],
/// [`RichEditor::pointer_double_click`], and [`RichEditor::pointer_scroll`].
pub struct RichEditor {
    state: Rc<RefCell<EditorState>>,
    document: RichDocument,
}

impl RichEditor {
    pub fn new(markdown: &str) -> Self {
        let document = RichDocument::from_markdown(markdown);
        let state = Rc::new(RefCell::new(EditorState::new(document.blocks())));
        Self { state, document }
    }

    /// Render the current editor contents to an RGBA image for Slint to display.
    pub fn surface(&mut self) -> Image {
        Image::from_rgba8(self.state.borrow_mut().render())
    }

    /// Return the editor's current plain-text contents (line-separated).
    pub fn text(&self) -> String {
        self.state.borrow().plain_text()
    }

    /// Serialize the current document back to markdown
    pub fn to_markdown(&self) -> String {
        self.document.to_markdown()
    }

    /// Return a short status string describing selection/caret.
    pub fn status(&self) -> String {
        self.state.borrow().status()
    }

    /// Handle a key event from Slint.
    pub fn input_key(&mut self, text: &str, shift: bool, control: bool, meta: bool, alt: bool) {
        self.state.borrow_mut().key(text, shift, control, meta, alt);
        self.sync_document_from_editor_text();
    }

    /// Handle a pointer down event at `(x, y)`. When `extend` is set (Shift
    /// held), the click extends the existing selection to the clicked point
    /// instead of collapsing the caret.
    pub fn pointer_down(&mut self, x: f32, y: f32, extend: bool) {
        self.state.borrow_mut().click(x, y, extend);
    }

    /// Handle a pointer drag event to `(x, y)`.
    pub fn pointer_drag(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().drag(x, y);
    }

    /// Handle a pointer double-click at `(x, y)` (select word).
    pub fn pointer_double_click(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().double_click(x, y);
    }

    /// Handle a pointer triple-click at `(x, y)` (select line/paragraph).
    pub fn pointer_triple_click(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().triple_click(x, y);
    }

    /// Handle a pointer scroll event by `pixels`.
    pub fn pointer_scroll(&mut self, pixels: f32) {
        self.state.borrow_mut().scroll(pixels);
    }

    /// Toggle bold formatting on the current selection.
    pub fn toggle_bold(&mut self) {
        self.state.borrow_mut().toggle_format_bold();
        self.sync_document_from_editor_text();
    }

    /// Toggle italic formatting on the current selection.
    pub fn toggle_italic(&mut self) {
        self.state.borrow_mut().toggle_format_italic();
        self.sync_document_from_editor_text();
    }

    /// Cut the current selection to the clipboard.
    pub fn cut_selection(&mut self) {
        self.state.borrow_mut().cut_selection();
        self.sync_document_from_editor_text();
    }

    /// Copy the current selection to the clipboard.
    pub fn copy_selection(&mut self) {
        self.state.borrow_mut().copy_selection();
    }

    /// Paste clipboard text at the caret.
    pub fn paste_clipboard(&mut self) {
        self.state.borrow_mut().paste_clipboard();
        self.sync_document_from_editor_text();
    }

    /// Undo the last edit.
    pub fn undo(&mut self) {
        self.state.borrow_mut().undo();
        self.sync_document_from_editor_text();
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) {
        self.state.borrow_mut().redo();
        self.sync_document_from_editor_text();
    }

    /// Select the whole document.
    pub fn select_all(&mut self) {
        self.state.borrow_mut().select_all();
    }

    /// Set whether the editor currently owns input focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.state.borrow_mut().set_focused(focused);
    }

    /// Return whether the editor currently owns input focus.
    pub fn is_focused(&self) -> bool {
        self.state.borrow().is_focused()
    }

    /// Caret geometry in surface coordinates, or `None` when unresolvable.
    pub fn caret_rect(&self) -> Option<(f32, f32, f32)> {
        self.state.borrow_mut().caret_rect()
    }

    fn sync_document_from_editor_text(&mut self) {
        let blocks = self.state.borrow().serialize_blocks();
        self.document = RichDocument::from_blocks(blocks);
    }
}

/// Minimal wrapper over Penumbra's markdown AST.
struct RichDocument {
    document: Document,
}

impl RichDocument {
    fn from_markdown(markdown: &str) -> Self {
        let document = parse_document(markdown).unwrap_or_else(|_| Document {
            blocks: vec![Block {
                id: BlockId::new(),
                kind: BlockKind::Paragraph(vec![Inline::Text(markdown.to_owned())]),
            }],
        });
        Self { document }
    }

    fn blocks(&self) -> &[Block] {
        &self.document.blocks
    }

    fn to_markdown(&self) -> String {
        render_markdown(&self.document)
    }

    fn from_blocks(blocks: Vec<Block>) -> Self {
        let blocks = if blocks.is_empty() {
            vec![Block {
                id: BlockId::new(),
                kind: BlockKind::Paragraph(Vec::new()),
            }]
        } else {
            blocks
        };
        Self {
            document: Document { blocks },
        }
    }
}
