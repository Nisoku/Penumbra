use cosmic_text::Color;
use slint::{Rgba8Pixel, SharedPixelBuffer};

/// Which inline mark a toggle request wraps the selection with.
#[derive(Clone, Copy)]
pub(super) enum Format {
    Bold,
    Italic,
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
