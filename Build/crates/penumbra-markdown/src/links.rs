//! Extraction of wikilinks and inline tags from parsed markdown.

use std::collections::HashSet;

use crate::ast::{Block, Document, Inline};

/// Collect wikilink targets referenced by a document, deduplicated in
/// first-appearance order. Aliases (`[[Target|alias]]`), heading anchors
/// (`[[Target#section]]`) and folder paths (`[[folder/Target]]`) reduce to
/// the bare note title.
pub fn extract_wikilinks(doc: &Document) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    collect_from_blocks(&doc.blocks, &mut |inline| {
        if let Inline::NoteEmbed { note_ref } = inline {
            let target = normalize_link_target(note_ref);
            if !target.is_empty() && seen.insert(target.clone()) {
                targets.push(target);
            }
        }
    });
    targets
}

/// Collect inline `#tag` names, deduplicated in first-appearance order.
pub fn extract_inline_tags(doc: &Document) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tags = Vec::new();
    collect_from_blocks(&doc.blocks, &mut |inline| {
        if let Inline::TagRef { name } = inline {
            let valid = !name.is_empty() && !name.chars().all(|c| c.is_ascii_digit());
            if valid && seen.insert(name.clone()) {
                tags.push(name.clone());
            }
        }
    });
    tags
}

/// Reduce a raw `NoteEmbed` reference to the bare note title used for
/// resolution against vault filenames.
pub fn normalize_link_target(note_ref: &str) -> String {
    let without_alias = note_ref.split('|').next().unwrap_or("");
    let without_anchor = without_alias.split('#').next().unwrap_or("");
    let last_segment = without_anchor.rsplit('/').next().unwrap_or("");
    last_segment.trim().to_string()
}

/// Rewrite inbound wikilinks in raw body text after a title change.
///
/// Handles both `[[Old]]` and `[[Old|alias]]` forms, case-insensitively,
/// preserving whatever case the author wrote.
pub fn rewrite_wikilink_targets(body: &str, old_title: &str, new_title: &str) -> String {
    if old_title == new_title {
        return body.to_string();
    }
    let lower_needle = format!("[[{}", old_title.to_lowercase());

    let processed: Vec<String> = body
        .split('\n')
        .scan(FenceState::default(), |state, line| {
            Some(rewrite_line(
                state,
                line,
                &lower_needle,
                old_title.len(),
                new_title,
            ))
        })
        .collect();
    processed.join("\n")
}

#[derive(Default)]
struct FenceState {
    inside: bool,
    marker: Option<&'static str>,
}

fn rewrite_line(
    state: &mut FenceState,
    line: &str,
    lower_needle: &str,
    old_len: usize,
    new_title: &str,
) -> String {
    let trimmed = line.trim_start();

    if state.inside {
        if let Some(marker) = state.marker {
            if trimmed.starts_with(marker) {
                state.inside = false;
                state.marker = None;
            }
        }
        return line.to_string();
    }
    if let Some(marker) = fence_opening(trimmed) {
        state.inside = true;
        state.marker = Some(marker);
        return line.to_string();
    }
    // Indented code blocks never contain live links either.
    if line.starts_with("    ") || line.starts_with('\t') {
        return line.to_string();
    }

    let lower_line = line.to_lowercase();
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0;
    let mut index = 0;

    while index < lower_line.len() {
        if lower_line.as_bytes()[index] == b'`' {
            // Inline code span content is inert; step past it untouched.
            if let Some(close) = lower_line[index + 1..].find('`') {
                index += close + 2;
                continue;
            }
        }
        if lower_line[index..].starts_with(lower_needle) {
            let after = lower_line
                .as_bytes()
                .get(index + lower_needle.len())
                .copied();
            if matches!(after, Some(b']') | Some(b'|')) {
                out.push_str(&line[cursor..index]);
                out.push_str("[[");
                out.push_str(new_title);
                cursor = index + "[[".len() + old_len;
                index = cursor;
                continue;
            }
        }
        index += 1;
    }
    out.push_str(&line[cursor..]);
    out
}

fn fence_opening(trimmed: &str) -> Option<&'static str> {
    ["```", "~~~"]
        .into_iter()
        .find(|marker| trimmed.starts_with(marker))
}

fn collect_from_blocks(blocks: &[Block], visit: &mut dyn FnMut(&Inline)) {
    for block in blocks {
        match block {
            Block::Paragraph(inlines)
            | Block::Heading {
                children: inlines, ..
            } => collect_from_inlines(inlines, visit),
            Block::List { items, .. } => {
                for item in items {
                    collect_from_blocks(&item.children, visit);
                }
            }
            Block::Quote(nested) => collect_from_blocks(nested, visit),
            Block::Table(table) => {
                for cell in &table.headers {
                    collect_from_inlines(cell, visit);
                }
                for row in &table.rows {
                    for cell in row {
                        collect_from_inlines(cell, visit);
                    }
                }
            }
            Block::HtmlBlock(_) => {}
            Block::FootnoteDefinition { children, .. } => collect_from_blocks(children, visit),
            Block::CodeBlock { .. } | Block::ThematicBreak => {}
        }
    }
}

fn collect_from_inlines(inlines: &[Inline], visit: &mut dyn FnMut(&Inline)) {
    for inline in inlines {
        visit(inline);
        match inline {
            Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Strikethrough(children)
            | Inline::Link { children, .. } => collect_from_inlines(children, visit),
            _ => {}
        }
    }
}
