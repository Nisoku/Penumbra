use cosmic_text::{Attrs, AttrsList, Color, Family, Metrics, Style, UnderlineStyle, Weight};
use penumbra_markdown::ast::{BlockKind, Inline, ListItem};

/// Body text size in points.
pub(super) const BODY_SIZE: f32 = 15.0;
/// Line height applied to body-size runs.
pub(super) const BODY_LINE: f32 = 24.0;

/// Point size for each heading level (index 0 == H1); levels past H6 reuse the
/// last entry.
const HEADING_SIZES: [f32; 6] = [28.0, 22.0, 19.0, 17.0, 16.0, 15.0];
/// Multiplier turning a heading point size into its line height.
const HEADING_LINE_SCALE: f32 = 1.28;

/// Ink used for ordinary body text.
const INK: Color = Color::rgb(0x1f, 0x1d, 0x1a);
/// Muted ink used for quotes, list markers, and table rules.
pub(super) const MUTED_INK: Color = Color::rgb(0x6a, 0x64, 0x5c);
/// Amber used for inline and block code.
const CODE_INK: Color = Color::rgb(0x8f, 0x6a, 0x20);
/// Indigo used for links.
const LINK_INK: Color = Color::rgb(0x3d, 0x56, 0xb0);

/// Spaces of indentation applied per level of nested list or quote.
const INDENT_UNIT: &str = "    ";

/// Inline formatting marks
#[derive(Clone, Copy, Default)]
pub(super) struct InlineMarks {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) code: bool,
    pub(super) link: bool,
    pub(super) strike: bool,
    /// Overrides the default body ink when set (quotes, markers).
    color: Option<Color>,
    /// Per-run metrics override
    metrics: Option<Metrics>,
}

impl InlineMarks {
    pub(super) fn code() -> Self {
        InlineMarks {
            code: true,
            ..InlineMarks::default()
        }
    }

    pub(super) fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

/// Turn Penumbra's parsed block AST into attributed display runs. Blocks are
/// separated by a blank line.
pub(super) fn markdown_runs(
    blocks: &[penumbra_markdown::ast::Block],
) -> Vec<(String, Attrs<'static>)> {
    let mut runs = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            push_run(&mut runs, "\n\n", InlineMarks::default());
        }
        append_block(block, 0, &mut runs);
    }
    runs
}

/// Append a single block at the given indentation depth
fn append_block(
    block: &penumbra_markdown::ast::Block,
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    match &block.kind {
        BlockKind::Paragraph(children) => {
            push_indent(depth, runs);
            append_inlines(children, InlineMarks::default(), runs);
        }
        BlockKind::Heading { level, children } => {
            push_indent(depth, runs);
            let size = HEADING_SIZES[(*level as usize).clamp(1, 6) - 1];
            append_inlines(
                children,
                InlineMarks {
                    bold: true,
                    ..InlineMarks::default()
                }
                .with_metrics(Metrics::new(size, size * HEADING_LINE_SCALE)),
                runs,
            );
        }
        BlockKind::CodeBlock { text, .. } => append_code_block(text, depth, runs),
        BlockKind::Quote(children) => append_quote(children, depth, runs),
        BlockKind::List {
            ordered,
            start,
            items,
        } => append_list(*ordered, *start, items, depth, runs),
        BlockKind::ThematicBreak => {
            push_indent(depth, runs);
            push_run(
                runs,
                "\u{2500}".repeat(48).as_str(),
                InlineMarks::default().with_color(MUTED_INK),
            );
        }
        BlockKind::HtmlBlock(html) => {
            push_indent(depth, runs);
            push_run(runs, html, InlineMarks::default().with_color(MUTED_INK));
        }
        BlockKind::Table(table) => super::runs_table::append_table(table, depth, runs),
        BlockKind::FootnoteDefinition { name, children } => {
            push_indent(depth, runs);
            push_run(
                runs,
                &format!("[^{name}]: "),
                InlineMarks::default().with_color(MUTED_INK),
            );
            append_child_blocks(children, depth, runs);
        }
    }
}

/// Append a sequence of child blocks that belong to a single parent (quote,
/// list item, footnote)
fn append_child_blocks(
    blocks: &[penumbra_markdown::ast::Block],
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            push_run(runs, "\n", InlineMarks::default());
        }
        append_block(block, depth, runs);
    }
}

/// Render a fenced or indented code block as monospace lines
fn append_code_block(text: &str, depth: usize, runs: &mut Vec<(String, Attrs<'static>)>) {
    let marks = InlineMarks {
        code: true,
        ..InlineMarks::default()
    }
    .with_color(CODE_INK);
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    for (index, line) in trimmed.split('\n').enumerate() {
        if index > 0 {
            push_run(runs, "\n", InlineMarks::default());
        }
        push_indent(depth, runs);
        push_run(runs, "  ", marks);
        if !line.is_empty() {
            push_run(runs, line, marks);
        }
    }
}

/// Render a block quote
fn append_quote(
    children: &[penumbra_markdown::ast::Block],
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    let bar = InlineMarks::default().with_color(MUTED_INK);
    let body = InlineMarks {
        italic: true,
        ..InlineMarks::default()
    }
    .with_color(MUTED_INK);
    for (index, block) in children.iter().enumerate() {
        if index > 0 {
            push_run(runs, "\n", InlineMarks::default());
        }
        push_indent(depth, runs);
        push_run(runs, "\u{2502} ", bar);
        match &block.kind {
            BlockKind::Paragraph(inlines)
            | BlockKind::Heading {
                children: inlines, ..
            } => append_inlines(inlines, body, runs),
            _ => append_block(block, depth, runs),
        }
    }
}

/// Render an ordered or unordered list
fn append_list(
    ordered: bool,
    start: Option<u64>,
    items: &[ListItem],
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    let mut number = start.unwrap_or(1);
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            push_run(runs, "\n", InlineMarks::default());
        }
        push_indent(depth, runs);
        let marker = match (ordered, item.checked) {
            (_, Some(true)) => "\u{2611} ".to_owned(),
            (_, Some(false)) => "\u{2610} ".to_owned(),
            (true, None) => {
                let marker = format!("{number}. ");
                number += 1;
                marker
            }
            (false, None) => "\u{2022} ".to_owned(),
        };
        push_run(runs, &marker, InlineMarks::default().with_color(MUTED_INK));
        append_item_blocks(&item.children, depth, runs);
    }
}

/// Append the blocks inside one list item
fn append_item_blocks(
    blocks: &[penumbra_markdown::ast::Block],
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    for (index, block) in blocks.iter().enumerate() {
        match (&block.kind, index) {
            (BlockKind::Paragraph(inlines), 0) => {
                append_inlines(inlines, InlineMarks::default(), runs)
            }
            _ => {
                push_run(runs, "\n", InlineMarks::default());
                append_block(block, depth + 1, runs);
            }
        }
    }
}

pub(super) fn push_indent(depth: usize, runs: &mut Vec<(String, Attrs<'static>)>) {
    if depth == 0 {
        return;
    }
    push_run(runs, &INDENT_UNIT.repeat(depth), InlineMarks::default());
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
            Inline::Strikethrough(children) => append_inlines(
                children,
                InlineMarks {
                    strike: true,
                    ..marks
                },
                runs,
            ),
            Inline::Code(text) => push_run(
                runs,
                text,
                InlineMarks {
                    code: true,
                    ..marks
                }
                .with_color(CODE_INK),
            ),
            Inline::Link { children, .. } => append_inlines(
                children,
                InlineMarks {
                    link: true,
                    ..marks
                },
                runs,
            ),
            Inline::Image { alt, .. } => push_run(
                runs,
                &format!("\u{1f5bc} {alt}"),
                InlineMarks {
                    italic: true,
                    ..marks
                }
                .with_color(MUTED_INK),
            ),
            Inline::NoteEmbed { note_ref } => push_run(
                runs,
                &format!("![[{note_ref}]]"),
                InlineMarks {
                    link: true,
                    ..marks
                },
            ),
            Inline::TagRef { name } => push_run(
                runs,
                &format!("#{name}"),
                InlineMarks {
                    link: true,
                    ..marks
                },
            ),
            Inline::InlineHtml(html) => push_run(runs, html, marks.with_color(MUTED_INK)),
            Inline::FootnoteRef(name) => {
                push_run(runs, &format!("[^{name}]"), marks.with_color(LINK_INK))
            }
            Inline::LineBreak => push_run(runs, "\n", marks),
            Inline::SoftBreak => push_run(runs, " ", marks),
        }
    }
}

/// Build the per-run [`Attrs`] for a text slice from its formatting marks and push onto runs
pub(super) fn push_run(runs: &mut Vec<(String, Attrs<'static>)>, text: &str, marks: InlineMarks) {
    if text.is_empty() {
        return;
    }
    let mut attrs = Attrs::new().family(if marks.code {
        Family::Monospace
    } else {
        Family::SansSerif
    });

    let color = marks
        .color
        .unwrap_or(if marks.link { LINK_INK } else { INK });
    attrs = attrs.color(color);

    if marks.link {
        attrs = attrs.underline(UnderlineStyle::Single);
    }
    if marks.strike {
        attrs = attrs.strikethrough();
    }
    if marks.bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if marks.italic {
        attrs = attrs.style(Style::Italic);
    }
    if let Some(metrics) = marks.metrics {
        attrs = attrs.metrics(metrics);
    }
    runs.push((text.to_owned(), attrs));
}

/// Read one buffer line's attributed spans into markdown inlines
pub(super) fn append_line_inlines(text: &str, attrs: &AttrsList, out: &mut Vec<Inline>) {
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
            strike: attr.text_decoration.strikethrough,
            ..InlineMarks::default()
        };
        match spans.last_mut() {
            Some((last_range, last_marks))
                if last_range.end == start
                    && last_marks.bold == marks.bold
                    && last_marks.italic == marks.italic
                    && last_marks.code == marks.code
                    && last_marks.strike == marks.strike =>
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
    if marks.strike {
        inline = Inline::Strikethrough(vec![inline]);
    }
    if marks.bold {
        inline = Inline::Strong(vec![inline]);
    }
    inline
}
