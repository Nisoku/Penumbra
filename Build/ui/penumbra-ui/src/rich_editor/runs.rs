use cosmic_text::{Attrs, AttrsList, Color, Family, Style, Weight};
use penumbra_markdown::ast::{BlockKind, Inline};

/// Inline formatting marks derived from the Markdown AST.
#[derive(Clone, Copy, Default)]
pub(super) struct InlineMarks {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) code: bool,
    pub(super) link: bool,
}

/// Turn Penumbra's parsed inline AST into attributed display runs.
pub(super) fn markdown_runs(
    blocks: &[penumbra_markdown::ast::Block],
) -> Vec<(String, Attrs<'static>)> {
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

fn append_inlines(inlines: &[Inline], marks: InlineMarks, runs: &mut Vec<(String, Attrs<'static>)>) {
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
            Inline::InlineHtml(html) => push_run(runs, html, marks),
            Inline::FootnoteRef(name) => push_run(runs, &format!("[^{name}]"), marks),
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
