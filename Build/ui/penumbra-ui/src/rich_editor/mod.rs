//! A COSMIC Text-based rich text editor rendered into a Slint surface.

mod engine;
mod format;
mod scan;

use engine::EditorState;
use slint::Image;
use std::{cell::RefCell, rc::Rc};

// TODO: make dynamic and responsive
const WIDTH: u32 = 940;
const HEIGHT: u32 = 560;
const CONTENT_PADDING_X: i32 = 30;
const CONTENT_PADDING_TOP: i32 = 22;
const CONTENT_PADDING_BOTTOM: i32 = 22;

/// A handle to the rich editor.
///
/// Creates an `Image` surface via [`RichEditor::surface`] that Slint displays,
/// and accepts input events through [`RichEditor::input_key`],
/// [`RichEditor::pointer_down`], [`RichEditor::pointer_drag`],
/// [`RichEditor::pointer_double_click`], and [`RichEditor::pointer_scroll`].
pub struct RichEditor {
    state: Rc<RefCell<EditorState>>,
}

impl RichEditor {
    pub fn new(markdown: &str) -> Self {
        let state = Rc::new(RefCell::new(EditorState::new(markdown)));
        Self { state }
    }

    /// Render the current editor contents to an RGBA image for Slint to display.
    pub fn surface(&mut self) -> Image {
        Image::from_rgba8(self.state.borrow_mut().render())
    }

    /// Render the current editor contents and return the raw RGBA bytes.
    #[doc(hidden)]
    pub fn surface_rgba(&mut self) -> Vec<u8> {
        self.state.borrow_mut().render().as_bytes().to_vec()
    }

    /// Return the editor's current contents.
    pub fn text(&self) -> String {
        self.state.borrow().source_text()
    }

    /// Return the current document as markdown. Byte-identical to [`text`], since
    /// the buffer *is* the markdown source.
    pub fn to_markdown(&self) -> String {
        self.state.borrow().source_text()
    }

    /// Return a short status string describing selection/caret.
    pub fn status(&self) -> String {
        self.state.borrow().status()
    }

    /// Handle a key event from Slint.
    pub fn input_key(&mut self, text: &str, shift: bool, control: bool, meta: bool, alt: bool) {
        self.state.borrow_mut().key(text, shift, control, meta, alt);
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
    }

    /// Toggle italic formatting on the current selection.
    pub fn toggle_italic(&mut self) {
        self.state.borrow_mut().toggle_format_italic();
    }

    /// Cut the current selection to the clipboard.
    pub fn cut_selection(&mut self) {
        self.state.borrow_mut().cut_selection();
    }

    /// Copy the current selection to the clipboard.
    pub fn copy_selection(&mut self) {
        self.state.borrow_mut().copy_selection();
    }

    /// Paste clipboard text at the caret.
    pub fn paste_clipboard(&mut self) {
        self.state.borrow_mut().paste_clipboard();
    }

    /// Undo the last edit.
    pub fn undo(&mut self) {
        self.state.borrow_mut().undo();
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) {
        self.state.borrow_mut().redo();
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
}
