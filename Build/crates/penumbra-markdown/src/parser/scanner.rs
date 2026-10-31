use crate::ast::Inline;

pub(super) fn split_text_for_custom(text: &str) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::new();
    let mut buf = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'[' && bytes.get(i + 1) == Some(&b'[') {
            if let Some(end) = find_closing_bracket(text, i + 2) {
                let ref_text = &text[i + 2..end];
                if !ref_text.is_empty() {
                    if !buf.is_empty() {
                        result.push(Inline::Text(std::mem::take(&mut buf)));
                    }
                    result.push(Inline::NoteEmbed {
                        note_ref: ref_text.to_string(),
                    });
                    i = end + 2;
                    continue;
                }
            }
        }

        if bytes[i] == b'#'
            && (i == 0 || is_tag_boundary(bytes[i - 1]))
            && text[i + 1..]
                .chars()
                .next()
                .is_some_and(|next| !next.is_whitespace() && next != '#')
        {
            let tag_end = find_tag_end(text, i + 1);
            if tag_end > i + 1 {
                if !buf.is_empty() {
                    result.push(Inline::Text(std::mem::take(&mut buf)));
                }
                result.push(Inline::TagRef {
                    name: text[i + 1..tag_end].to_string(),
                });
                i = tag_end;
                continue;
            }
        }

        let ch = text[i..].chars().next().expect("i is a char boundary");
        buf.push(ch);
        i += ch.len_utf8();
    }

    if !buf.is_empty() {
        result.push(Inline::Text(buf));
    }

    result
}

fn find_closing_bracket(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b']' {
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    None
}

fn find_tag_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for c in text[start..].chars() {
        // Unicode letters keep non-ASCII tags intact;
        // slashes allow nested tags like #projects/penumbra.
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '/' {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn is_tag_boundary(b: u8) -> bool {
    b.is_ascii_whitespace()
        || b == b'('
        || b == b'['
        || b == b','
        || b == b'.'
        || b == b'!'
        || b == b'?'
        || b == b':'
        || b == b';'
}
