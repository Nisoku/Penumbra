//! Presentation rendering of editor blocks for the panel.

use penumbra_editor::doc::BlockKind;
use penumbra_editor::session::BlockEdit;

const AVG_CHAR_PX: f32 = 7.8;
const MONO_CHAR_PX: f32 = 7.5;
const MONO_FONT_PX: f32 = 12.5;
const BODY_FONT_PX: f32 = 15.0;

/// The machine name of a block kind, used to route blocks to their preview
/// component in the panel.
pub fn kind_name(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph(_) => "paragraph",
        BlockKind::Heading { .. } => "heading",
        BlockKind::Quote(_) => "quote",
        BlockKind::List { .. } => "list",
        BlockKind::CodeBlock { .. } => "code",
        BlockKind::ThematicBreak => "break",
        BlockKind::Table(_) => "table",
        BlockKind::HtmlBlock(_) => "html",
        BlockKind::FootnoteDefinition { .. } => "footnote",
    }
}

/// The zero-based heading level, 0 for non-headings.
pub fn heading_level(block: &BlockEdit) -> u8 {
    match &block.kind {
        BlockKind::Heading { level, .. } => *level,
        _ => 0,
    }
}

/// The routing kind the currently typed text would render as, used to preview
/// a block live while the user edits it.
pub fn live_kind_name(kind: &BlockKind, text: &str) -> &'static str {
    match kind {
        BlockKind::Paragraph(_) | BlockKind::Quote(_) | BlockKind::List { .. } => {
            let t = text.trim_start();
            if t.starts_with('#') {
                "heading"
            } else if t.starts_with('>') {
                "quote"
            } else if t.starts_with("- ")
                || t.starts_with("+ ")
                || t.starts_with("* ")
                || t.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && t.chars().nth(1) == Some('.')
            {
                "list"
            } else {
                kind_name(kind)
            }
        }
        _ => kind_name(kind),
    }
}

/// The heading level the typed text would render at (count of leading `#`),
/// bounded to the 6 markdown levels.
pub fn live_heading_level(kind: &BlockKind, text: &str) -> u8 {
    if live_kind_name(kind, text) == "heading" {
        let hashes = text.trim_start().chars().take_while(|&c| c == '#').count();
        hashes.clamp(1, 6) as u8
    } else {
        1
    }
}

/// The text rendered in the live preview for the typed `text` of `kind`.
pub fn live_display_text(kind: &BlockKind, text: &str) -> String {
    match live_kind_name(kind, text) {
        "heading" => sanitize_inline(&strip_prefix_markers(text.trim_start(), '#')),
        "quote" => sanitize_inline(&strip_prefix_lines(text, '>')),
        "code" => code_parts(text).0,
        "footnote" => strip_first_prefix(text, '['),
        "table" => render_table(text),
        "html" => String::new(),
        "break" => String::new(),
        _ => text.to_owned(),
    }
}

/// The text shown in an inactive block's preview.
pub fn display_text(block: &BlockEdit) -> String {
    match &block.kind {
        BlockKind::Heading { .. } => {
            sanitize_inline(&strip_prefix_markers(block.text.trim_start(), '#'))
        }
        BlockKind::Quote(_) => sanitize_inline(&strip_prefix_lines(&block.text, '>')),
        BlockKind::CodeBlock { .. } => code_parts(&block.text).0,
        BlockKind::Table(_) => render_table(&block.text),
        BlockKind::FootnoteDefinition { .. } => strip_first_prefix(&block.text, '['),
        BlockKind::HtmlBlock(_) => String::new(),
        _ => block.text.clone(),
    }
}

/// The language tag of a fenced code block, empty when absent.
pub fn code_language(block: &BlockEdit) -> String {
    match &block.kind {
        BlockKind::CodeBlock { .. } => code_parts(&block.text).1,
        _ => String::new(),
    }
}

/// Rendered body plus language tag of a fenced (or indented) code block.
fn code_parts(raw: &str) -> (String, String) {
    let mut lines = raw.lines();
    let Some(first) = lines.next() else {
        return (String::new(), String::new());
    };

    let trimmed = first.trim_start();
    let Some(info) = trimmed.strip_prefix('`').map(|s| s.trim()) else {
        return (raw.trim_end().to_owned(), String::new());
    };
    if !info.starts_with('`') {
        return (raw.trim_end().to_owned(), String::new());
    }

    let language = info
        .trim_start_matches('`')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();

    let mut body = String::new();
    for line in lines {
        if line.trim_end().ends_with('`') {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    (body.trim_end().to_owned(), language)
}

/// Strip a `prefix` marker from the start of each line (quote `>`).
fn strip_prefix_lines(text: &str, prefix: char) -> String {
    text.lines()
        .map(|line| {
            let stripped = line.trim_start();
            stripped
                .strip_prefix(prefix)
                .map(str::trim_start)
                .unwrap_or(stripped)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip a run of `marker` chars from a string's start.
fn strip_prefix_markers(text: &str, marker: char) -> String {
    text.trim_start_matches(marker).trim_start().to_owned()
}

/// Strip a `[^label]: ` definition prefix from the text.
fn strip_first_prefix(text: &str, marker: char) -> String {
    let t = text.trim_start();
    let Some(after_open) = t.strip_prefix(marker) else {
        return text.to_owned();
    };
    let Some(feet) = after_open.find(']') else {
        return text.to_owned();
    };
    let rest = after_open[feet + 1..].trim_start();
    rest.strip_prefix(':')
        .map(str::trim_start)
        .unwrap_or(rest)
        .to_owned()
}

/// Reduce inline markdown to plain text for previews that cannot render it
/// (weighted headings, italics quotes): emphasis markers are dropped, code
/// spans keep their content, and `[label](url)` links reduce to `label`.
fn sanitize_inline(src: &str) -> String {
    let cs: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '`' => {
                i += 1;
                while i < cs.len() && cs[i] != '`' {
                    out.push(cs[i]);
                    i += 1;
                }
                if i < cs.len() {
                    i += 1;
                }
            }
            '[' => {
                let close = cs[i + 1..]
                    .iter()
                    .position(|&k| k == ']')
                    .map(|p| i + 1 + p);
                if let Some(close_i) = close {
                    if cs.get(close_i + 1) == Some(&'(') {
                        let open = close_i + 2;
                        if let Some(paren_end) = cs[open..].iter().position(|&k| k == ')') {
                            out.extend(cs[i + 1..close_i].iter());
                            i = open + paren_end + 1;
                            continue;
                        }
                    }
                }
                out.push(c);
                i += 1;
            }
            '*' | '_' | '~' => {
                let mut j = i;
                while j < cs.len() && cs[j] == c && j - i < 2 {
                    j += 1;
                }
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Render a markdown table as a left-aligned monospace grid.
///
/// The separator line is dropped; a fresh dashes row underlines the header so
/// the table still reads as a table in a plain preview.
fn render_table(raw: &str) -> String {
    const MAX_CHARS: usize = 88;

    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in raw.lines() {
        if let Some(cells) = split_row(line) {
            if !is_separator_row(&cells) {
                rows.push(cells);
            }
        }
    }
    let Some(header) = rows.first() else {
        return raw.to_owned();
    };

    let columns = header.len();
    let too_short = rows.iter().any(|r| r.len() < columns);
    if columns == 0 || too_short {
        return raw.to_owned();
    }

    let mut widths = vec![0usize; columns];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    const PAD: usize = 1;
    let total = widths.iter().map(|w| w + 2 * PAD).sum::<usize>() + (columns - 1) * 3;
    if total > MAX_CHARS {
        let overflow = (total - MAX_CHARS) as f32;
        let sum_widths = widths.iter().sum::<usize>() as f32;
        if sum_widths > 0.0 {
            let scale = 1.0 - overflow / sum_widths;
            for w in &mut widths {
                *w = ((*w as f32) * scale).round().max(3.0) as usize;
            }
        }
    }

    let mut out = String::new();
    push_padded_row(&mut out, header, &widths);
    out.push('\n');
    let separator = widths
        .iter()
        .map(|&w| "-".repeat(w + 2 * PAD))
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str(&separator);
    for row in &rows[1..] {
        out.push('\n');
        push_padded_row(&mut out, row, &widths);
    }
    out
}

/// Split one table row into its trimmed cells; `None` for non-row lines.
fn split_row(line: &str) -> Option<Vec<String>> {
    let mut s = line.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(stripped) = s.strip_prefix('|') {
        s = stripped;
    }
    if let Some(stripped) = s.strip_suffix('|') {
        s = stripped;
    }
    if !s.contains('|') {
        return None;
    }
    Some(s.split('|').map(|c| c.trim().to_owned()).collect())
}

/// True when every cell is a dashes-only alignment marker (`:-:` etc).
fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim();
            let inner = t.trim_start_matches(':').trim_end_matches(':');
            !inner.is_empty() && inner.bytes().all(|b| b == b'-')
        })
}

/// Write one row as padded, left-aligned cells separated by ` | `.
fn push_padded_row(out: &mut String, row: &[String], widths: &[usize]) {
    let mut first = true;
    for (i, cell) in row.iter().enumerate() {
        if !first {
            out.push_str(" | ");
        }
        first = false;
        let width = widths.get(i).copied().unwrap_or(0);
        let pad = width.saturating_sub(cell.chars().count());
        out.push_str(cell);
        out.extend(std::iter::repeat_n(' ', pad + 2 * PAD));
    }
}

const PAD: usize = 1;

/// Approximate line-count of a wrapped preview string for a given column.
fn wrapped_lines(text: &str, font_px: f32, char_px: f32, column_px: f32) -> usize {
    let _ = font_px;
    let per_line = ((column_px - 16.0) / char_px).max(8.0) as usize;
    text.lines()
        .map(|line| {
            let chars = line.chars().count();
            chars.div_ceil(per_line).max(1)
        })
        .sum()
}

/// Best-effort height of a block's preview, mirroring the panel's paddings so
/// the auto-scroll lands close to the real layout.
pub fn estimate_block_height(block: &BlockEdit, column_px: f32) -> f32 {
    match &block.kind {
        BlockKind::Heading { level, .. } => {
            let size = match level {
                1 => 29.0,
                2 => 26.0,
                _ => 23.0,
            };
            let lines = wrapped_lines(&display_text(block), size, AVG_CHAR_PX, column_px);
            size * lines as f32 + 16.0
        }
        BlockKind::CodeBlock { .. } => {
            let (body, lang) = code_parts(&block.text);
            let lines = wrapped_lines(&body, MONO_FONT_PX, MONO_CHAR_PX, column_px);
            MONO_FONT_PX * 1.35 * lines as f32 + if lang.is_empty() { 24.0 } else { 42.0 }
        }
        BlockKind::Table(_) => {
            let body = render_table(&block.text);
            let lines = wrapped_lines(&body, MONO_FONT_PX, MONO_CHAR_PX, column_px);
            MONO_FONT_PX * 1.35 * lines as f32 + 24.0
        }
        BlockKind::Quote(_) => {
            let lines = wrapped_lines(&display_text(block), BODY_FONT_PX, AVG_CHAR_PX, column_px);
            BODY_FONT_PX * 1.45 * lines as f32 + 8.0
        }
        BlockKind::HtmlBlock(_) => 30.0,
        BlockKind::FootnoteDefinition { .. } => {
            let lines = wrapped_lines(&display_text(block), 13.0, AVG_CHAR_PX, column_px);
            13.0 * 1.4 * lines as f32 + 8.0
        }
        _ => {
            let lines = wrapped_lines(&display_text(block), BODY_FONT_PX, AVG_CHAR_PX, column_px);
            BODY_FONT_PX * 1.45 * lines as f32 + 8.0
        }
    }
}

/// Approximate total height of the active block's live preview
pub fn estimate_live_height(kind: &BlockKind, text: &str, column_px: f32) -> f32 {
    let input = match kind {
        BlockKind::CodeBlock { .. } => 168.0,
        BlockKind::Heading { .. } => 48.0,
        _ => 96.0,
    };
    if text.is_empty() {
        return input + 32.0;
    }
    let live_kind = live_kind_name(kind, text);
    let live_text = live_display_text(kind, text);
    let preview = match live_kind {
        "heading" => {
            let size = match live_heading_level(kind, text) {
                1 => 29.0,
                2 => 26.0,
                _ => 23.0,
            };
            let lines = wrapped_lines(&live_text, size, AVG_CHAR_PX, column_px);
            size * lines as f32 + 12.0
        }
        "code" | "table" => {
            let lines = wrapped_lines(&live_text, MONO_FONT_PX, MONO_CHAR_PX, column_px);
            MONO_FONT_PX * 1.35 * lines as f32 + 12.0
        }
        "quote" => {
            let lines = wrapped_lines(&live_text, BODY_FONT_PX, AVG_CHAR_PX, column_px);
            BODY_FONT_PX * 1.45 * lines as f32 + 12.0
        }
        "footnote" => {
            let lines = wrapped_lines(&live_text, 13.0, AVG_CHAR_PX, column_px);
            13.0 * 1.4 * lines as f32 + 12.0
        }
        "break" | "html" => 26.0,
        _ => {
            let lines = wrapped_lines(&live_text, BODY_FONT_PX, AVG_CHAR_PX, column_px);
            BODY_FONT_PX * 1.45 * lines as f32 + 12.0
        }
    };
    input + 32.0 + preview
}

/// The scroll offset that brings the active block to the vertical center of
/// the panel body, clamped to the top.
pub fn scroll_target_y(blocks: &[BlockEdit], active: usize, column_px: f32, body_px: f32) -> f32 {
    const PADDING_TOP: f32 = 28.0;
    const SPACING: f32 = 4.0;

    let mut y = PADDING_TOP;
    for block in blocks.iter().take(active) {
        y += estimate_block_height(block, column_px) + SPACING;
    }
    let active_height = blocks
        .get(active)
        .map(|b| estimate_block_height(b, column_px))
        .unwrap_or(0.0);
    (y - (body_px - active_height) / 2.0).max(0.0)
}
