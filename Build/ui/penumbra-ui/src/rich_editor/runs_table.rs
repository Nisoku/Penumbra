use cosmic_text::Attrs;
use penumbra_markdown::ast::{Inline, TableAlign};

use super::runs::{push_indent, push_run, InlineMarks, MUTED_INK};

/// Minimum column width in characters
const MIN_COLUMN_WIDTH: usize = 3;

/// Render a GFM table as a proper monospace grid
pub(super) fn append_table(
    table: &penumbra_markdown::ast::Table,
    depth: usize,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    let column_count = table
        .headers
        .len()
        .max(table.rows.iter().map(Vec::len).max().unwrap_or(0));
    if column_count == 0 {
        return;
    }

    let header_cells: Vec<String> = (0..column_count)
        .map(|col| table.headers.get(col).map(cell_text).unwrap_or_default())
        .collect();
    let body_cells: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            (0..column_count)
                .map(|col| row.get(col).map(cell_text).unwrap_or_default())
                .collect()
        })
        .collect();

    let widths: Vec<usize> = (0..column_count)
        .map(|col| {
            let header_width = header_cells[col].chars().count();
            let body_width = body_cells
                .iter()
                .map(|row| row[col].chars().count())
                .max()
                .unwrap_or(0);
            header_width.max(body_width).max(MIN_COLUMN_WIDTH)
        })
        .collect();

    let grid = InlineMarks::code().with_color(MUTED_INK);
    let cell = InlineMarks::code();

    emit_table_row(
        &header_cells,
        &widths,
        &table.align,
        depth,
        grid,
        cell,
        runs,
    );
    push_run(runs, "\n", InlineMarks::default());
    emit_table_rule(&widths, &table.align, depth, grid, runs);
    for row in &body_cells {
        push_run(runs, "\n", InlineMarks::default());
        emit_table_row(row, &widths, &table.align, depth, grid, cell, runs);
    }
}

/// Emit one padded, pipe-delimited table row.
fn emit_table_row(
    cells: &[String],
    widths: &[usize],
    align: &[TableAlign],
    depth: usize,
    grid: InlineMarks,
    cell: InlineMarks,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    push_indent(depth, runs);
    push_run(runs, "\u{2502} ", grid);
    for (col, text) in cells.iter().enumerate() {
        if col > 0 {
            push_run(runs, " \u{2502} ", grid);
        }
        let column_align = align.get(col).copied().unwrap_or(TableAlign::None);
        push_run(runs, &pad_cell(text, widths[col], column_align), cell);
    }
    push_run(runs, " \u{2502}", grid);
}

/// Emit the horizontal rule separating the header from the body
fn emit_table_rule(
    widths: &[usize],
    align: &[TableAlign],
    depth: usize,
    grid: InlineMarks,
    runs: &mut Vec<(String, Attrs<'static>)>,
) {
    push_indent(depth, runs);
    // ugh i know, raw unicode? well this is literally why this one exists (i think)
    push_run(runs, "\u{2502}", grid);
    for (col, width) in widths.iter().enumerate() {
        if col > 0 {
            push_run(runs, "\u{2502}", grid);
        }
        let column_align = align.get(col).copied().unwrap_or(TableAlign::None);
        let (left, right) = match column_align {
            TableAlign::Left => (':', '\u{2500}'),
            TableAlign::Right => ('\u{2500}', ':'),
            TableAlign::Center => (':', ':'),
            TableAlign::None => ('\u{2500}', '\u{2500}'),
        };
        let mut rule = String::new();
        rule.push(left);
        rule.push_str(&"\u{2500}".repeat(*width));
        rule.push(right);
        push_run(runs, &rule, grid);
    }
    push_run(runs, "\u{2502}", grid);
}

/// Pad `text` to `width` columns
fn pad_cell(text: &str, width: usize, align: TableAlign) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_owned();
    }
    let pad = width - len;
    match align {
        TableAlign::Right => format!("{}{}", " ".repeat(pad), text),
        TableAlign::Center => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
        TableAlign::Left | TableAlign::None => format!("{}{}", text, " ".repeat(pad)),
    }
}

/// Flatten a table cell's inlines to plain text for grid alignment.
fn cell_text(cell: &Vec<Inline>) -> String {
    let mut text = String::new();
    for inline in cell {
        inline.write_plain_text(&mut text);
    }
    text
}
