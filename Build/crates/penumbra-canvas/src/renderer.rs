use crate::state::RenderState;

pub trait GraphCanvasRenderer {
    fn new() -> Self;
    fn resize(&mut self, width: f32, height: f32);
    fn render(&mut self, state: &RenderState);
    fn set_theme(&mut self, css_vars: &str);
}

/// No-op renderer for when the platform-specific implementation
/// is not compiled. Falls back to background colour fill.
pub struct NullCanvasRenderer;

impl GraphCanvasRenderer for NullCanvasRenderer {
    fn new() -> Self {
        Self
    }

    fn resize(&mut self, _width: f32, _height: f32) {}

    fn render(&mut self, _state: &RenderState) {}

    fn set_theme(&mut self, _css_vars: &str) {}
}
