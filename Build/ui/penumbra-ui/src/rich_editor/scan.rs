use cosmic_text::{Attrs, AttrsList, Color, Family, Metrics, Style, Weight};
use penumbra_markdown::ast::BlockKind;
use penumbra_markdown::parser::parse_blocks_offsets;

pub(super) const BODY_SIZE: f32 = 15.0;
pub(super) const BODY_LINE: f32 = 24.0;

// Point size per heading level (index 0 == H1); past H6 reuses the last entry.
const HEADING_SIZES: [f32; 6] = [28.0, 22.0, 19.0, 17.0, 16.0, 15.0];
const HEADING_LINE_SCALE: f32 = 1.28;

const INK: Color = Color::rgb(0x1f, 0x1d, 0x1a);
const MUTED_INK: Color = Color::rgb(0x6a, 0x64, 0x5c);
const CODE_INK: Color = Color::rgb(0x8f, 0x6a, 0x20);

// How the parser classified the block a given line belongs to.
#[derive(Clone, Copy, Default)]
pub(super) enum LineKind {
    #[default]
    Body,
    Heading(u8),
    Code,
    Quote,
    List,
}

pub(super) fn body_metrics() -> Metrics {
    Metrics::new(BODY_SIZE, BODY_LINE)
}

fn base_attrs() -> Attrs<'static> {
    Attrs::new()
        .family(Family::SansSerif)
        .color(INK)
        .metrics(body_metrics())
}

// Classify every line of `source` by its owning block, so restyle can look up a
// line's kind without re-parsing per line and being... slow
pub(super) fn classify_lines(source: &str) -> Vec<LineKind> {
    let line_count = source.split('\n').count();
    let mut kinds = vec![LineKind::Body; line_count];

    let Ok(blocks) = parse_blocks_offsets(source) else {
        return kinds;
    };
    for (block, (start, end)) in blocks {
        let kind = match block.kind {
            BlockKind::Heading { level, .. } => LineKind::Heading(level),
            BlockKind::CodeBlock { .. } => LineKind::Code,
            BlockKind::Quote(_) => LineKind::Quote,
            BlockKind::List { .. } => LineKind::List,
            _ => LineKind::Body,
        };
        for line in line_span(source, start, end) {
            if let Some(slot) = kinds.get_mut(line) {
                *slot = kind;
            }
        }
    }
    kinds
}

// Inclusive line indices a byte range spans
fn line_span(source: &str, start: usize, end: usize) -> std::ops::RangeInclusive<usize> {
    let first = source[..start.min(source.len())].matches('\n').count();
    let last_byte = end.min(source.len()).saturating_sub(1);
    let last = source[..=last_byte.min(source.len().saturating_sub(1))]
        .matches('\n')
        .count();
    first..=last.max(first)
}

// Style one line given its parser-classified kind. The returned list spans
// exactly `0..line.len()`; the text is never modified.
pub(super) fn scan_line(line: &str, kind: LineKind) -> AttrsList {
    let mut attrs = AttrsList::new(&base_attrs());
    let len = line.len();
    if len == 0 {
        return attrs;
    }

    match kind {
        LineKind::Code => {
            attrs.add_span(0..len, &mono(ink_for_fence(line)));
            attrs
        }
        LineKind::Heading(level) => {
            let size = HEADING_SIZES[(level as usize).clamp(1, 6) - 1];
            attrs.add_span(0..len, &heading_attrs(size));
            scan_inline_spans(line, &mut attrs, Some(size));
            attrs
        }
        LineKind::Quote | LineKind::List => {
            if let Some(prefix) = block_marker_end(line) {
                attrs.add_span(0..prefix, &marker_attrs());
            }
            scan_inline_spans(line, &mut attrs, None);
            attrs
        }
        LineKind::Body => {
            scan_inline_spans(line, &mut attrs, None);
            attrs
        }
    }
}

// A fence delimiter is drawn muted for... obvious reasons
fn ink_for_fence(line: &str) -> Color {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        MUTED_INK
    } else {
        CODE_INK
    }
}

fn mono(color: Color) -> Attrs<'static> {
    Attrs::new()
        .family(Family::Monospace)
        .color(color)
        .metrics(body_metrics())
}

fn heading_attrs(size: f32) -> Attrs<'static> {
    Attrs::new()
        .family(Family::SansSerif)
        .color(INK)
        .weight(Weight::BOLD)
        .metrics(Metrics::new(size, size * HEADING_LINE_SCALE))
}

fn marker_attrs() -> Attrs<'static> {
    Attrs::new()
        .family(Family::SansSerif)
        .color(MUTED_INK)
        .metrics(body_metrics())
}

// Byte offset past a leading quote/list marker (including indentation), else None.
fn block_marker_end(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    if let Some(after) = rest.strip_prefix("> ") {
        return Some(line.len() - after.len());
    }
    for bullet in ["- ", "* ", "+ "] {
        if let Some(after) = rest.strip_prefix(bullet) {
            return Some(line.len() - after.len());
        }
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 {
        let tail = &rest[digits..];
        if tail.starts_with(". ") || tail.starts_with(") ") {
            return Some(indent + digits + 2);
        }
    }
    None
}

// Overlay inline emphasis spans onto `attrs`, markers kept in the text. The
// parser does not give inline byte offsets, so this is the one place we scan
// raw. `heading_size` keeps spans at the heading metric when set.
fn scan_inline_spans(line: &str, attrs: &mut AttrsList, heading_size: Option<f32>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some((end, kind)) = match_inline(bytes, i) {
            attrs.add_span(i..end, &inline_attrs(kind, heading_size));
            i = end;
        } else {
            i += 1;
        }
    }
}

#[derive(Clone, Copy)]
enum InlineKind {
    Bold,
    Italic,
    Code,
    Strike,
}

// If an inline construct opens at `start`, return its end (past the closing
// marker) and kind. Single-line only.
fn match_inline(bytes: &[u8], start: usize) -> Option<(usize, InlineKind)> {
    let opens = |marker: &[u8]| bytes[start..].starts_with(marker);
    if opens(b"**") {
        return close(bytes, start + 2, b"**").map(|e| (e, InlineKind::Bold));
    }
    if opens(b"~~") {
        return close(bytes, start + 2, b"~~").map(|e| (e, InlineKind::Strike));
    }
    if opens(b"`") {
        return close(bytes, start + 1, b"`").map(|e| (e, InlineKind::Code));
    }
    // A lone `*`/`_` needs a non-space after it to open italic.
    if (opens(b"*") || opens(b"_")) && bytes.get(start + 1).is_some_and(|&b| b != b' ') {
        let marker = &bytes[start..start + 1];
        return close(bytes, start + 1, marker).map(|e| (e, InlineKind::Italic));
    }
    None
}

// Offset just past the closing `marker` at or after `from`, else None.
fn close(bytes: &[u8], from: usize, marker: &[u8]) -> Option<usize> {
    let mut i = from;
    while i + marker.len() <= bytes.len() {
        if bytes[i..].starts_with(marker) {
            return Some(i + marker.len());
        }
        i += 1;
    }
    None
}

fn inline_attrs(kind: InlineKind, heading_size: Option<f32>) -> Attrs<'static> {
    let metrics = match heading_size {
        Some(size) => Metrics::new(size, size * HEADING_LINE_SCALE),
        None => body_metrics(),
    };
    let base = Attrs::new()
        .family(Family::SansSerif)
        .color(INK)
        .metrics(metrics);
    match kind {
        InlineKind::Bold => base.weight(Weight::BOLD),
        InlineKind::Italic => base.style(Style::Italic),
        InlineKind::Strike => base.strikethrough(),
        InlineKind::Code => Attrs::new()
            .family(Family::Monospace)
            .color(CODE_INK)
            .metrics(metrics),
    }
}
