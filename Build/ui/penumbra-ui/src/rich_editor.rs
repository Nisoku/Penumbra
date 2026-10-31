//! A COSMIC Text-based rich text editor rendered into a Slint surface

use cosmic_text::{
    Action, Attrs, AttrsList, AttrsOwned, Buffer, Color, Cursor, Edit, Editor, Family, FontSystem,
    Metrics, Motion, Selection, Shaping, Style, SwashCache, Weight,
};
use penumbra_markdown::ast::{Block, BlockId, BlockKind, Document, Inline};
use penumbra_markdown::parser::parse_document;
use penumbra_markdown::render::markdown::render_markdown;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
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
    /// Create a new rich editor from a Markdown string.
    pub fn new(markdown: &str) -> Self {
        let document = RichDocument::from_markdown(markdown);
        let mut font_system = FontSystem::new();
        let cache = SwashCache::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(18.0, 29.0));
        buffer.set_size(
            Some((WIDTH as i32 - CONTENT_PADDING_X * 2) as f32),
            Some((HEIGHT as i32 - CONTENT_PADDING_TOP - CONTENT_PADDING_BOTTOM) as f32),
        );
        let rich_runs = markdown_runs(document.blocks());
        buffer.set_rich_text(
            rich_runs
                .iter()
                .map(|(text, attrs)| (text.as_str(), attrs.clone())),
            &Attrs::new(),
            Shaping::Advanced,
            None,
        );

        let state = Rc::new(RefCell::new(EditorState {
            font_system,
            cache,
            editor: Editor::new(buffer),
            undo: Vec::new(),
            redo: Vec::new(),
            open_undo_group: None,
            notice: None,
            focused: false,
        }));

        Self { state, document }
    }

    /// Render the current editor contents to an RGBA image for Slint to display.
    pub fn surface(&mut self) -> Image {
        Image::from_rgba8(self.state.borrow_mut().render())
    }

    /// Return the editor's current plain-text contents (line-separated).
    pub fn text(&self) -> String {
        self.state
            .borrow()
            .editor
            .with_buffer(|buffer| {
                let mut out = String::new();
                for (i, line) in buffer.lines.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    out.push_str(line.text());
                }
                out
            })
            .into()
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
    pub fn input_key(&mut self, text: &str, shift: bool, control: bool, meta: bool) {
        self.state.borrow_mut().key(text, shift, control, meta);
        self.sync_document_from_editor_text();
    }

    /// Handle a pointer down event at `(x, y)`.
    pub fn pointer_down(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().click(x, y);
    }

    /// Handle a pointer drag event to `(x, y)`.
    pub fn pointer_drag(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().drag(x, y);
    }

    /// Handle a pointer double-click at `(x, y)`.
    pub fn pointer_double_click(&mut self, x: f32, y: f32) {
        self.state.borrow_mut().double_click(x, y);
    }

    /// Handle a pointer scroll event by `pixels`.
    pub fn pointer_scroll(&mut self, pixels: f32) {
        self.state
            .borrow_mut()
            .scroll(pixels);
    }

    /// Toggle bold formatting on the current selection.
    pub fn toggle_bold(&mut self) {
        self.state.borrow_mut().toggle_format(Format::Bold);
        self.sync_document_from_editor_text();
    }

    /// Toggle italic formatting on the current selection.
    pub fn toggle_italic(&mut self) {
        self.state.borrow_mut().toggle_format(Format::Italic);
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

    /// Paste the clipboard contents at the caret.
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

    /// Select all text.
    pub fn select_all(&mut self) {
        self.state.borrow_mut().select_all();
    }

    /// Mark the editor focused or unfocused.
    pub fn set_focused(&mut self, focused: bool) {
        self.state.borrow_mut().focused = focused;
    }

    /// Whether the editor currently holds focus.
    pub fn is_focused(&self) -> bool {
        self.state.borrow().focused
    }

    /// Caret geometry in surface pixels: `(x, y, height)`. Returns zeros when
    /// the caret has no resolvable layout position.
    pub fn caret_rect(&self) -> (f32, f32, f32) {
        self.state
            .borrow_mut()
            .caret_rect()
            .unwrap_or((CONTENT_PADDING_X as f32, CONTENT_PADDING_TOP as f32, 29.0))
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

/// Turn Penumbra's parsed inline AST into attributed display runs.
fn markdown_runs(blocks: &[penumbra_markdown::ast::Block]) -> Vec<(String, Attrs<'static>)> {
    let mut runs = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            push_run(&mut runs, "\n\n", InlineMarks::default());
        }
        match &block.kind {
            BlockKind::Paragraph(children) => {
                append_inlines(children, InlineMarks::default(), &mut runs)
            }
            BlockKind::Heading { children, .. } => append_inlines(
                children,
                InlineMarks {
                    bold: true,
                    ..InlineMarks::default()
                },
                &mut runs,
            ),
            BlockKind::CodeBlock { text, .. } => push_run(
                &mut runs,
                text,
                InlineMarks {
                    code: true,
                    ..InlineMarks::default()
                },
            ),
            BlockKind::Quote(children) => append_blocks(children, &mut runs),
            BlockKind::List { items, .. } => {
                for (item_index, item) in items.iter().enumerate() {
                    if item_index > 0 {
                        push_run(&mut runs, "\n", InlineMarks::default());
                    }
                    push_run(&mut runs, "• ", InlineMarks::default());
                    append_blocks(&item.children, &mut runs);
                }
            }
            BlockKind::ThematicBreak => push_run(&mut runs, "────────", InlineMarks::default()),
            BlockKind::HtmlBlock(html) => push_run(&mut runs, html, InlineMarks::default()),
            BlockKind::Table(table) => {
                for (row_index, row) in table.rows.iter().enumerate() {
                    if row_index > 0 {
                        push_run(&mut runs, "\n", InlineMarks::default());
                    }
                    for (cell_index, cell) in row.iter().enumerate() {
                        if cell_index > 0 {
                            push_run(&mut runs, "  ·  ", InlineMarks::default());
                        }
                        append_inlines(cell, InlineMarks::default(), &mut runs);
                    }
                }
            }
            BlockKind::FootnoteDefinition { name, children } => {
                push_run(&mut runs, &format!("[^{name}] "), InlineMarks::default());
                append_blocks(children, &mut runs);
            }
        }
    }
    runs
}

fn append_blocks(
    blocks: &[penumbra_markdown::ast::Block],
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            push_run(runs, "\n", InlineMarks::default());
        }
        match &block.kind {
            BlockKind::Paragraph(children) | BlockKind::Heading { children, .. } => {
                append_inlines(children, InlineMarks::default(), runs)
            }
            BlockKind::CodeBlock { text, .. } => push_run(
                runs,
                text,
                InlineMarks {
                    code: true,
                    ..InlineMarks::default()
                },
            ),
            BlockKind::Quote(children) => append_blocks(children, runs),
            _ => {
                let mut text = String::new();
                block.kind.write_plain_text(&mut text);
                push_run(runs, &text, InlineMarks::default());
            }
        }
    }
}

fn append_inlines(
    inlines: &[Inline],
    marks: InlineMarks,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => push_run(runs, text, marks),
            Inline::Strong(children) => append_inlines(
                children,
                InlineMarks {
                    bold: true,
                    ..marks
                },
                runs,
            ),
            Inline::Emphasis(children) => append_inlines(
                children,
                InlineMarks {
                    italic: true,
                    ..marks
                },
                runs,
            ),
            Inline::Strikethrough(children) => append_inlines(children, marks, runs),
            Inline::Code(text) => push_run(
                runs,
                text,
                InlineMarks {
                    code: true,
                    ..marks
                },
            ),
            Inline::Link { children, .. } => append_inlines(
                children,
                InlineMarks {
                    link: true,
                    ..marks
                },
                runs,
            ),
            Inline::Image { alt, .. } => push_run(runs, alt, marks),
            Inline::NoteEmbed { note_ref } => push_run(runs, note_ref, marks),
            Inline::TagRef { name } => push_run(runs, &format!("#{name}"), marks),
            Inline::LineBreak => push_run(runs, "\n", marks),
            Inline::SoftBreak => push_run(runs, " ", marks),
        }
    }
}

fn push_run(runs: &mut Vec<(String, Attrs<'static>)>, text: &str, marks: InlineMarks) {
    if text.is_empty() {
        return;
    }
    let mut attrs = Attrs::new().family(if marks.code {
        Family::Monospace
    } else {
        Family::SansSerif
    });
    if marks.code {
        attrs = attrs.color(Color::rgb(0x8f, 0x6a, 0x20));
    } else if marks.link {
        attrs = attrs
            .color(Color::rgb(0x3d, 0x56, 0xb0))
            .underline(cosmic_text::UnderlineStyle::Single);
    } else {
        attrs = attrs.color(Color::rgb(0x1f, 0x1d, 0x1a));
    }
    if marks.bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if marks.italic {
        attrs = attrs.style(Style::Italic);
    }
    runs.push((text.to_owned(), attrs));
}

/// Read one buffer line's attributed spans into markdown inlines
fn append_line_inlines(text: &str, attrs: &AttrsList, out: &mut Vec<Inline>) {
    if text.is_empty() {
        return;
    }

    let mut spans: Vec<(std::ops::Range<usize>, InlineMarks)> = Vec::new();
    for (range, owned) in attrs.spans_iter() {
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        if start >= end {
            continue;
        }
        let attr = owned.as_attrs();
        let marks = InlineMarks {
            bold: attr.weight == Weight::BOLD,
            italic: attr.style == Style::Italic,
            code: matches!(attr.family, Family::Monospace),
            link: false,
        };
        match spans.last_mut() {
            Some((last_range, last_marks))
                if last_range.end == start
                    && last_marks.bold == marks.bold
                    && last_marks.italic == marks.italic
                    && last_marks.code == marks.code =>
            {
                last_range.end = end;
            }
            _ => spans.push((start..end, marks)),
        }
    }

    if spans.is_empty() {
        out.push(Inline::Text(text.to_owned()));
        return;
    }

    // Any text not covered by a span (e.g. trailing insert with no attrs) is
    // emitted as plain text so no characters are dropped (this was pain to find)
    let mut cursor = 0usize;
    for (range, marks) in spans {
        if range.start > cursor {
            out.push(Inline::Text(text[cursor..range.start].to_owned()));
        }
        let slice = text[range.start..range.end].to_owned();
        out.push(inline_from_marks(slice, marks));
        cursor = range.end;
    }
    if cursor < text.len() {
        out.push(Inline::Text(text[cursor..].to_owned()));
    }
}

fn inline_from_marks(text: String, marks: InlineMarks) -> Inline {
    if marks.code {
        return Inline::Code(text);
    }
    let mut inline = Inline::Text(text);
    if marks.italic {
        inline = Inline::Emphasis(vec![inline]);
    }
    if marks.bold {
        inline = Inline::Strong(vec![inline]);
    }
    inline
}

struct EditorState {
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

#[derive(Clone, Copy)]
enum Format {
    Bold,
    Italic,
}

impl EditorState {
    fn click(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        self.editor.action(
            &mut self.font_system,
            Action::Click {
                x: x.round() as i32,
                y: y.round() as i32,
            },
        );
    }

    fn drag(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        self.editor.action(
            &mut self.font_system,
            Action::Drag {
                x: x.round() as i32,
                y: y.round() as i32,
            },
        );
    }

    fn double_click(&mut self, x: f32, y: f32) {
        self.close_undo_group();
        self.editor.action(
            &mut self.font_system,
            Action::DoubleClick {
                x: x.round() as i32,
                y: y.round() as i32,
            },
        );
    }

    fn scroll(&mut self, pixels: f32) {
        self.editor
            .action(&mut self.font_system, Action::Scroll { pixels });
    }

    fn key(&mut self, text: &str, shift: bool, control: bool, meta: bool) {
        if control || meta {
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
                "y" if control => self.redo(),
                _ => {}
            }
            return;
        }

        let action = match text {
            "Backspace" => Some(Action::Backspace),
            "Delete" => Some(Action::Delete),
            "Enter" => Some(Action::Enter),
            "Left" => Some(Action::Motion(Motion::Left)),
            "Right" => Some(Action::Motion(Motion::Right)),
            "Up" => Some(Action::Motion(Motion::Up)),
            "Down" => Some(Action::Motion(Motion::Down)),
            "Home" => Some(Action::Motion(Motion::Home)),
            "End" => Some(Action::Motion(Motion::End)),
            "Escape" => Some(Action::Escape),
            _ => None,
        };

        if text == "Tab" {
            self.close_undo_group();
            return;
        }

        if let Some(action) = action {
            if matches!(action, Action::Escape) {
                self.close_undo_group();
                self.editor.action(&mut self.font_system, action);
                return;
            }
            if shift && matches!(action, Action::Motion(_)) {
                if matches!(self.editor.selection(), Selection::None) {
                    self.editor
                        .set_selection(Selection::Normal(self.editor.cursor()));
                }
            } else if !shift {
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

    fn select_all(&mut self) {
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

    fn undo(&mut self) {
        self.close_undo_group();
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.editor.clone());
            self.editor = previous;
        }
    }

    fn redo(&mut self) {
        self.close_undo_group();
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.editor.clone());
            self.editor = next;
        }
    }

    fn copy_selection(&mut self) {
        let Some(text) = self.editor.copy_selection() else {
            self.notice = Some("Nothing selected to copy".into());
            return;
        };
        self.notice = match write_clipboard(&text) {
            Ok(()) => Some("Copied selection".into()),
            Err(error) => Some(format!("Clipboard error: {error}")),
        };
    }

    fn cut_selection(&mut self) {
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

    fn paste_clipboard(&mut self) {
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

    fn toggle_format(&mut self, format: Format) {
        let Some((start, end)) = self.editor.selection_bounds() else {
            return;
        };
        if start == end {
            return;
        }

        self.remember_undo();
        self.editor.with_buffer_mut(|buffer| {
            for line_index in start.line..=end.line {
                let line = &mut buffer.lines[line_index];
                let text_len = line.text().len();
                let selected_start = if line_index == start.line {
                    start.index
                } else {
                    0
                };
                let selected_end = if line_index == end.line {
                    end.index
                } else {
                    text_len
                };
                if selected_start >= selected_end {
                    continue;
                }

                let old_attrs = line.attrs_list().clone();
                let mut boundaries = vec![0, text_len, selected_start, selected_end];
                for (range, _) in old_attrs.spans_iter() {
                    boundaries.push(range.start);
                    boundaries.push(range.end);
                }
                boundaries.sort_unstable();
                boundaries.dedup();

                let mut new_attrs = AttrsList::new(&old_attrs.defaults());
                for pair in boundaries.windows(2) {
                    let (from, to) = (pair[0], pair[1]);
                    if from >= to || to > text_len {
                        continue;
                    }
                    let mut attrs = AttrsOwned::new(&old_attrs.get_span(from));
                    if from >= selected_start && to <= selected_end {
                        match format {
                            Format::Bold => {
                                attrs.weight = if attrs.weight == Weight::BOLD {
                                    Weight::NORMAL
                                } else {
                                    Weight::BOLD
                                };
                            }
                            Format::Italic => {
                                attrs.style = if attrs.style == Style::Italic {
                                    Style::Normal
                                } else {
                                    Style::Italic
                                };
                            }
                        }
                    }
                    new_attrs.add_span(from..to, &attrs.as_attrs());
                }
                line.set_attrs_list(new_attrs);
            }
            buffer.set_redraw(true);
        });
    }

    /// Serialize the live buffer into markdown blocks, preserving inline marks
    fn serialize_blocks(&self) -> Vec<Block> {
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

    fn status(&self) -> String {
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

    fn render(&mut self) -> SharedPixelBuffer<Rgba8Pixel> {
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
    fn caret_rect(&mut self) -> Option<(f32, f32, f32)> {
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

fn write_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|error| error.to_string())
}

fn read_clipboard() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard.get_text().map_err(|error| error.to_string())
}

fn blend_rect(
    pixels: &mut SharedPixelBuffer<Rgba8Pixel>,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    let width = pixels.width() as i32;
    let height = pixels.height() as i32;
    let bytes = color.as_rgba();
    let dst = pixels.make_mut_slice();
    for py in y.max(0)..(y + h as i32).min(height) {
        for px in x.max(0)..(x + w as i32).min(width) {
            let i = py as usize * width as usize + px as usize;
            let alpha = bytes[3] as u32;
            let inv = 255 - alpha;
            dst[i].r = ((bytes[0] as u32 * alpha + dst[i].r as u32 * inv) / 255) as u8;
            dst[i].g = ((bytes[1] as u32 * alpha + dst[i].g as u32 * inv) / 255) as u8;
            dst[i].b = ((bytes[2] as u32 * alpha + dst[i].b as u32 * inv) / 255) as u8;
            dst[i].a = (alpha + dst[i].a as u32 * inv / 255) as u8;
        }
    }
}

/// Inline formatting marks derived from the Markdown AST.
#[derive(Clone, Copy, Default)]
struct InlineMarks {
    bold: bool,
    italic: bool,
    code: bool,
    link: bool,
}
