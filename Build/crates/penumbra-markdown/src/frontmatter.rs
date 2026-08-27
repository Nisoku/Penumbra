//! YAML frontmatter codec for vault note files.

use chrono::{DateTime, Utc};
use penumbra_core::error::{PenumbraError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FENCE_OPEN: &str = "---";

/// Identity and metadata carried in a note file's frontmatter block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub id: Uuid,
    #[serde(rename = "created")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updated")]
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub archived: bool,
}

/// A parsed note file: optional frontmatter plus the markdown body.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNoteFile {
    pub frontmatter: Option<Frontmatter>,
    pub body: String,
}

/// Serialize frontmatter to a fenced block ending with a newline.
pub fn serialize(fm: &Frontmatter) -> Result<String> {
    let inner = serde_norway::to_string(fm)
        .map_err(|e| PenumbraError::Markdown(format!("frontmatter emit: {e}")))?;
    Ok(format!("{FENCE_OPEN}\n{inner}{FENCE_OPEN}\n"))
}

/// Split a note file into optional frontmatter and body.
///
/// Errors when a frontmatter block exists but fails YAML parsing or lacks
/// required identity fields; a missing block yields `frontmatter: None`
/// with the full text as body.
pub fn parse(text: &str) -> Result<ParsedNoteFile> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().peekable();
    if lines.peek().map(|l| l.trim_end()) != Some(FENCE_OPEN) {
        return Ok(ParsedNoteFile {
            frontmatter: None,
            body: text.to_string(),
        });
    }
    lines.next();

    let mut inner = String::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == FENCE_OPEN {
            closed = true;
            break;
        }
        inner.push_str(line);
        inner.push('\n');
    }
    if !closed {
        return Err(PenumbraError::Markdown(
            "unterminated frontmatter block".to_string(),
        ));
    }

    let fm: Frontmatter = serde_norway::from_str(inner.trim_end())
        .map_err(|e| PenumbraError::Markdown(format!("frontmatter: {e}")))?;

    let mut body = lines.collect::<Vec<_>>().join("\n");
    if !body.is_empty() {
        body.push('\n');
    }

    Ok(ParsedNoteFile {
        frontmatter: Some(fm),
        body,
    })
}

fn is_false(value: &bool) -> bool {
    !*value
}
