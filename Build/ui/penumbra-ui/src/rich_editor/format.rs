use cosmic_text::{AttrsList, AttrsOwned, Buffer, Color, Cursor, Style, Weight};
use slint::{Rgba8Pixel, SharedPixelBuffer};

/// Which inline attribute a toggle request flips.
#[derive(Clone, Copy)]
pub(super) enum Format {
    Bold,
    Italic,
}

/// Flip `format` across the `[start, end)` selection, splitting existing attr
/// spans on the selection boundaries so only the selected sub-range changes.
pub(super) fn apply_span_format(buffer: &mut Buffer, start: Cursor, end: Cursor, format: Format) {
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
}

/// Alpha-blend a solid `color` rectangle into the pixel buffer, clipped to the
/// buffer bounds.
pub(super) fn blend_rect(
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
