use dioxus::prelude::*;
use penumbra_canvas::{GraphCanvasRenderer, RenderState, WebCanvasRenderer};
use std::cell::RefCell;

thread_local! {
    static RENDERER: RefCell<Option<WebCanvasRenderer>> = const { RefCell::new(None) };
}

/// Attach a WebCanvasRenderer to a `<canvas>` and re-render whenever
/// `render_state` changes.
///
/// Call once inside the component that owns the canvas element.
pub fn use_canvas_render(canvas_id: &str, render_state: Signal<RenderState>) {
    let id = canvas_id.to_string();

    use_effect(move || {
        let state = render_state();
        RENDERER.with(|rc| {
            let rc = &mut *rc.borrow_mut();
            if rc.is_none() {
                *rc = Some(WebCanvasRenderer::new().with_canvas_id(&id));
            }
            if let Some(r) = rc {
                r.render(&state);
            }
        });
    });
}

/// Resize the canvas element and renderer when the window size changes.
pub fn use_canvas_resize(canvas_id: &str, width: f32, height: f32) {
    let id = canvas_id.to_string();
    let prev = use_signal(|| (0f32, 0f32));

    use_effect(move || {
        if *prev.read() == (width, height) {
            return;
        }
        *prev.write() = (width, height);

        RENDERER.with(|rc| {
            if let Some(r) = rc.borrow_mut().as_mut() {
                r.resize(width, height);
            }
        });
        // Also resize the DOM canvas element
        if let Some(window) = web_sys::window() {
            if let Some(doc) = window.document() {
                if let Some(el) = doc.get_element_by_id(&id) {
                    el.set_attribute("width", &width.to_string()).ok();
                    el.set_attribute("height", &height.to_string()).ok();
                }
            }
        }
    });
}
