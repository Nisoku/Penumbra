//! Filename derivation for vault note files.

const MAX_STEM_CHARS: usize = 100;
const FALLBACK_STEM: &str = "Untitled";

/// Convert a note title into a safe file stem.
pub fn sanitize_title_to_stem(title: &str) -> String {
    let replaced = sanitize_filename::sanitize_with_options(
        title,
        sanitize_filename::Options {
            windows: true,
            truncate: false,
            replacement: "-",
        },
    );

    let mut stem = String::with_capacity(replaced.len());
    let mut prev_space = false;
    for ch in replaced.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                stem.push(' ');
                prev_space = true;
            }
        } else {
            stem.push(ch);
            prev_space = false;
        }
    }

    let stem = stem.trim();
    let stem = stem.trim_start_matches('.');
    let stem = stem.trim_end_matches(['.', ' ']);
    let capped: String = stem.chars().take(MAX_STEM_CHARS).collect();
    if capped.is_empty() {
        return FALLBACK_STEM.to_string();
    }
    capped
}

/// Append numeric suffixes until the stem is unused.
pub fn dedupe_stem(mut candidate: String, taken: &mut dyn FnMut(&str) -> bool) -> String {
    let base = candidate.clone();
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{base} {n}");
        n += 1;
    }
    candidate
}
