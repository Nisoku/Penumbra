use cosmic_text::{
    Action, Buffer, Color, Cursor, Edit, Editor, FontSystem, Metrics, Motion, Selection, Shaping,
    SwashCache,
};
use penumbra_markdown::ast::{Block, BlockId, BlockKind, Inline};
use slint::{Rgba8Pixel, SharedPixelBuffer};

use super::format::{apply_span_format, blend_rect, Format};
use super::runs::{append_line_inlines, markdown_runs, BODY_LINE, BODY_SIZE};
use super::{CONTENT_PADDING_BOTTOM, CONTENT_PADDING_TOP, CONTENT_PADDING_X, HEIGHT, WIDTH};

pub(super) struct EditorState {
    font_system: FontSystem,
    cache: SwashCache,
    editor: Editor<'static>,
    undo: Vec<Editor<'static>>,
    redo: Vec<Editor<'static>>,
    open_undo_group: Option<UndoGroup>,
    notice: Option<String>,
    focused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UndoGroup {
    Typing,
    Backspace,
    Delete,
}

impl EditorState {
    /// Build the engine from the parsed document blocks, shaping the rich runs
    /// into the sized buffer the editor draws from.
    pub(super) fn new(blocks: &[Block]) -> Self {
        let mut font_system = FontSystem::new();
        let cache = SwashCache::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(BODY_SIZE, BODY_LINE));
        buffer.set_size(
            Some((WIDTH as i32 - CONTENT_PADDING_X * 2) as f32),
            Some((HEIGHT as i32 - CONTENT_PADDING_TOP - CONTENT_PADDING_BOTTOM) as f32),
        );
        let rich_runs = markdown_runs(blocks);
        buffer.set_rich_text(
            rich_runs
                .iter()
                .map(|(text, attrs)| (text.as_str(), attrs.clone())),
            &cosmic_text::Attrs::new(),
            Shaping::Advanced,
            None,
        );
        Self {
            font_system,
            cache,
            editor: Editor::new(buffer),
            undo: Vec::new(),
            redo: Vec::new(),
            open_undo_group: None,
            notice: None,
            focused: false,
        }
    }

    pub(super) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(super) fn is_focused(&self) -> bool {
        self.focused
    }

    /// Read the buffer's plain text, newline-joined.
    pub(super) fn plain_text(&self) -> String {
        self.editor.with_buffer(|buffer| {
            let mut out = String::new();
            for (i, line) in buffer.lines.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(line.text());
            }
            out
        })
    }

    /// Convert a surface-space pointer coordinate to the buffer-relative
    /// coordinate space cosmic-text hit-tests against.
    fn surface_to_buffer(x: f32, y: f32) -> (i32, i32) {
        (
            (x - CONTENT_PADDING_X as f32).round() as i32,
            (y - CONTENT_PADDING_TOP as f32).round() as i32,
        )
    }

    pub(super) fn click(&mut self, x: f32, y: f32, extend: bool) {
        self.close_undo_group();
        let (x, y) = Self::surface_to_buffer(x, y);
        if extend {
            if matches!(self.editor.selection(), Selection::None) {
                self.editor
                    .set_selection(Selection::Normal(self.editor.cursor()));
            }
            self.editor
                .action(&mut self.font_system, Action::Drag { x, y });
        } else {
            self.editor
                .action(&mut self.font_system, Action::Click { x, y });
        }
    }

    pub(super) fn drag(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        let (x, y) = Self::surface_to_buffer(x, y);
        self.editor
            .action(&mut self.font_system, Action::Drag { x, y });
    }

    pub(super) fn double_click(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        let (x, y) = Self::surface_to_buffer(x, y);
        self.editor
            .action(&mut self.font_system, Action::DoubleClick { x, y });
    }

    pub(super) fn triple_click(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        let (x, y) = Self::surface_to_buffer(x, y);
        self.editor
            .action(&mut self.font_system, Action::TripleClick { x, y });
    }

    pub(super) fn scroll(&mut self, pixels: f32) {
        self.editor
            .action(&mut self.font_system, Action::Scroll { pixels });
    }

    pub(super) fn key(&mut self, text: &str, shift: bool, control: bool, meta: bool, alt: bool) {
        // The primary edit modifier is Cmd on macOS (`meta` here) or Ctrl
        // elsewhere (`control`); either drives the editing shortcuts.
        if control || meta {
            self.handle_shortcut(text, shift);
            return;
        }

        // Option/Alt is a word-motion modifier on the arrow keys only.
        if alt {
            match text {
                "Left" => return self.motion_key(Motion::LeftWord, shift),
                "Right" => return self.motion_key(Motion::RightWord, shift),
                _ => {}
            }
        }

        self.handle_plain_key(text, shift);
    }

    /// Dispatch a shortcut keyed with the primary edit modifier held.
    fn handle_shortcut(&mut self, text: &str, shift: bool) {
        self.close_undo_group();
        match text.to_ascii_lowercase().as_str() {
            "a" => self.select_all(),
            "b" => self.toggle_format(Format::Bold),
            "i" => self.toggle_format(Format::Italic),
            "c" => self.copy_selection(),
            "x" => self.cut_selection(),
            "v" => self.paste_clipboard(),
            "z" if shift => self.redo(),
            "z" => self.undo(),
            "y" => self.redo(),
            "left" => self.motion_key(Motion::LeftWord, shift),
            "right" => self.motion_key(Motion::RightWord, shift),
            "up" | "home" => self.motion_key(Motion::BufferStart, shift),
            "down" | "end" => self.motion_key(Motion::BufferEnd, shift),
            "backspace" => {
                self.motion_key(Motion::LeftWord, true);
                self.edit_action(Action::Backspace);
            }
            "delete" => {
                self.motion_key(Motion::RightWord, true);
                self.edit_action(Action::Delete);
            }
            _ => {}
        }
    }

    /// Handle an unmodified key: a named editing action, or a literal insert.
    fn handle_plain_key(&mut self, text: &str, shift: bool) {
        let action = match text {
            "Backspace" => Some(Action::Backspace),
            "Delete" => Some(Action::Delete),
            "Enter" => Some(Action::Enter),
            "Left" => Some(Action::Motion(Motion::Left)),
            "Right" => Some(Action::Motion(Motion::Right)),
            "Up" => Some(Action::Motion(Motion::Up)),
            "Down" => Some(Action::Motion(Motion::Down)),
            "Home" => Some(Action::Motion(Motion::SoftHome)),
            "End" => Some(Action::Motion(Motion::End)),
            "PageUp" => Some(Action::Motion(Motion::PageUp)),
            "PageDown" => Some(Action::Motion(Motion::PageDown)),
            "Tab" => Some(Action::Indent),
            "BackTab" => Some(Action::Unindent),
            "Escape" => Some(Action::Escape),
            _ => None,
        };

        if let Some(action) = action {
            if matches!(action, Action::Escape) {
                self.close_undo_group();
                self.editor.action(&mut self.font_system, action);
                return;
            }
            if text == "Tab" && shift {
                self.close_undo_group();
                self.editor.set_selection(Selection::None);
                self.editor.action(&mut self.font_system, Action::Unindent);
                return;
            }
            if shift && matches!(action, Action::Motion(_)) {
                if matches!(self.editor.selection(), Selection::None) {
                    self.editor
                        .set_selection(Selection::Normal(self.editor.cursor()));
                }
            } else if !shift && matches!(action, Action::Motion(_)) {
                self.editor.set_selection(Selection::None);
            }
            self.edit_action(action);
            return;
        }

        if text.chars().any(char::is_control) {
            return;
        }
        if matches!(
            text.to_ascii_lowercase().as_str(),
            "shift" | "meta" | "control" | "alt" | "super"
        ) {
            return;
        }
        for character in text.chars() {
            self.edit_action(Action::Insert(character));
        }
    }

    /// Apply a cursor [`Motion`], extending the selection when `shift` is held
    /// and collapsing it otherwise
    fn motion_key(&mut self, motion: Motion, shift: bool) {
        if shift {
            if matches!(self.editor.selection(), Selection::None) {
                self.editor
                    .set_selection(Selection::Normal(self.editor.cursor()));
            }
        } else {
            self.editor.set_selection(Selection::None);
        }
        self.edit_action(Action::Motion(motion));
    }

    pub(super) fn select_all(&mut self) {
        self.close_undo_group();
        let end = self.editor.with_buffer(|buffer| {
            let line = buffer.lines.len().saturating_sub(1);
            Cursor::new(line, buffer.lines[line].text().len())
        });
        self.editor.set_cursor(end);
        self.editor
            .set_selection(Selection::Normal(Cursor::new(0, 0)));
    }

    fn edit_action(&mut self, action: Action) {
        match action {
            Action::Insert(_) => self.remember_undo_group(UndoGroup::Typing),
            Action::Backspace => self.remember_undo_group(UndoGroup::Backspace),
            Action::Delete => self.remember_undo_group(UndoGroup::Delete),
            Action::Enter => {
                self.close_undo_group();
                self.remember_undo();
            }
            _ => self.close_undo_group(),
        }
        self.editor.action(&mut self.font_system, action);
    }

    pub(super) fn undo(&mut self) {
        self.close_undo_group();
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.editor.clone());
            self.editor = previous;
        }
    }

    pub(super) fn redo(&mut self) {
        self.close_undo_group();
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.editor.clone());
            self.editor = next;
        }
    }

    pub(super) fn copy_selection(&mut self) {
        let Some(text) = self.editor.copy_selection() else {
            self.notice = Some("Nothing selected to copy".into());
            return;
        };
        self.notice = match write_clipboard(&text) {
            Ok(()) => Some("Copied selection".into()),
            Err(error) => Some(format!("Clipboard error: {error}")),
        };
    }

    pub(super) fn cut_selection(&mut self) {
        let Some(text) = self.editor.copy_selection() else {
            self.notice = Some("Nothing selected to cut".into());
            return;
        };
        if let Err(error) = write_clipboard(&text) {
            self.notice = Some(format!("Clipboard error: {error}"));
            return;
        }
        self.remember_undo();
        self.editor.delete_selection();
        self.notice = Some("Cut selection".into());
    }

    pub(super) fn paste_clipboard(&mut self) {
        match read_clipboard() {
            Ok(text) if !text.is_empty() => {
                self.remember_undo();
                self.editor.insert_string(&text, None);
                self.notice = Some("Pasted clipboard text".into());
            }
            Ok(_) => self.notice = Some("Clipboard is empty".into()),
            Err(error) => self.notice = Some(format!("Clipboard error: {error}")),
        }
    }

    fn remember_undo(&mut self) {
        self.close_undo_group();
        self.undo.push(self.editor.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn remember_undo_group(&mut self, group: UndoGroup) {
        if self.open_undo_group != Some(group) {
            self.undo.push(self.editor.clone());
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
            self.open_undo_group = Some(group);
        }
        self.redo.clear();
    }

    fn close_undo_group(&mut self) {
        self.open_undo_group = None;
    }

    pub(super) fn toggle_format_bold(&mut self) {
        self.toggle_format(Format::Bold);
    }

    pub(super) fn toggle_format_italic(&mut self) {
        self.toggle_format(Format::Italic);
    }

    fn toggle_format(&mut self, format: Format) {
        let Some((start, end)) = self.editor.selection_bounds() else {
            return;
        };
        if start == end {
            return;
        }
        self.remember_undo();
        self.editor
            .with_buffer_mut(|buffer| apply_span_format(buffer, start, end, format));
    }

    /// Serialize the live buffer into markdown blocks, preserving inline marks
    pub(super) fn serialize_blocks(&self) -> Vec<Block> {
        self.editor.with_buffer(|buffer| {
            let mut blocks = Vec::new();
            let mut paragraph: Vec<Inline> = Vec::new();

            let flush = |paragraph: &mut Vec<Inline>, blocks: &mut Vec<Block>| {
                if paragraph.is_empty() {
                    return;
                }
                blocks.push(Block {
                    id: BlockId::new(),
                    kind: BlockKind::Paragraph(std::mem::take(paragraph)),
                });
            };

            for line in buffer.lines.iter() {
                let text = line.text();
                if text.trim().is_empty() {
                    flush(&mut paragraph, &mut blocks);
                    continue;
                }
                if !paragraph.is_empty() {
                    paragraph.push(Inline::SoftBreak);
                }
                append_line_inlines(text, line.attrs_list(), &mut paragraph);
            }
            flush(&mut paragraph, &mut blocks);
            blocks
        })
    }

    pub(super) fn status(&self) -> String {
        let cursor = self.editor.cursor();
        let selection = match self.editor.selection_bounds() {
            Some((start, end)) if start != end => format!(
                "Selection: line {} byte {} -> line {} byte {}   |   caret: line {} byte {}",
                start.line + 1,
                start.index,
                end.line + 1,
                end.index,
                cursor.line + 1,
                cursor.index,
            ),
            _ => format!(
                "No selection   |   caret: line {} byte {}",
                cursor.line + 1,
                cursor.index,
            ),
        };
        match &self.notice {
            Some(notice) => format!("{notice}   |   {selection}"),
            None => selection,
        }
    }

    pub(super) fn render(&mut self) -> SharedPixelBuffer<Rgba8Pixel> {
        let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(WIDTH, HEIGHT);
        pixels.make_mut_slice().fill(Rgba8Pixel {
            r: 0xf7,
            g: 0xf5,
            b: 0xf0,
            a: 0xff,
        });
        self.editor.draw(
            &mut self.font_system,
            &mut self.cache,
            Color::rgb(0x1f, 0x1d, 0x1a),
            Color::rgba(0x3d, 0x56, 0xb0, 0x00),
            Color::rgb(0xc7, 0xd2, 0xf2),
            Color::rgb(0x1f, 0x1d, 0x1a),
            |x, y, w, h, color| {
                blend_rect(
                    &mut pixels,
                    x + CONTENT_PADDING_X,
                    y + CONTENT_PADDING_TOP,
                    w,
                    h,
                    color,
                )
            },
        );
        pixels
    }

    /// Caret geometry in surface coordinates
    pub(super) fn caret_rect(&mut self) -> Option<(f32, f32, f32)> {
        let cursor = self.editor.cursor();
        let font_system = &mut self.font_system;
        self.editor
            .with_buffer_mut(|buffer| buffer.shape_until_scroll(font_system, false));
        self.editor.with_buffer(|buffer| {
            for run in buffer.layout_runs() {
                if run.line_i != cursor.line {
                    continue;
                }
                let x = run.cursor_position(&cursor).unwrap_or(0.0);
                return Some((
                    x + CONTENT_PADDING_X as f32,
                    run.line_top + CONTENT_PADDING_TOP as f32,
                    run.line_height,
                ));
            }
            None
        })
    }
}

#[cfg(not(target_family = "wasm"))]
fn write_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|error| error.to_string())
}

#[cfg(not(target_family = "wasm"))]
fn read_clipboard() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard.get_text().map_err(|error| error.to_string())
}

// The browser Clipboard API is async-only which is annoying sometimes
#[cfg(target_family = "wasm")]
fn write_clipboard(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no browser window")?;
    let _ = window.navigator().clipboard().write_text(text);
    Ok(())
}

#[cfg(target_family = "wasm")]
fn read_clipboard() -> Result<String, String> {
    Err("paste is handled by the browser on the web build".into())
}
