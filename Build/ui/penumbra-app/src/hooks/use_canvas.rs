use dioxus::prelude::*;
use penumbra_canvas::RenderState;
use tracing;

#[cfg(not(target_arch = "wasm32"))]
pub fn use_canvas(
    canvas_id: &str,
    render_state: Signal<RenderState>,
    size: Signal<(f32, f32)>,
    ready: Signal<bool>,
) {
    let id = canvas_id.to_string();
    let mut initted = use_signal(|| false);

    let init_id = id.clone();

    // Set up canvas sizing and the draw function once
    use_effect(move || {
        if !ready() || *initted.read() {
            return;
        }
        *initted.write() = true;
        _ = document::eval(&format!(
            "window.__penumbra_canvas_id='{init_id}';{}",
            include_str!("../../assets/canvas-draw.js")
        ));
    });

    // Resize canvas when window size changes
    let resize_id = id;
    use_effect(move || {
        if !ready() {
            return;
        }
        let (w, h) = *size.read();
        _ = document::eval(&format!(
            "const c=document.getElementById('{resize_id}');if(c){{c.width={w};c.height={h};}}"
        ));
    });

    // Push render state + draw on every change
    use_effect(move || {
        if !ready() {
            return;
        }
        let state = render_state();
        match serde_json::to_string(&state) {
            Ok(json) => {
                _ = document::eval(&format!(
                    "window.__penumbra_state={json};window.__penumbra_draw();"
                ));
            }
            Err(e) => {
                tracing::error!("failed to serialize render state: {e}");
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
pub fn use_canvas(
    canvas_id: &str,
    render_state: Signal<RenderState>,
    size: Signal<(f32, f32)>,
    ready: Signal<bool>,
) {
    use penumbra_canvas::{GraphCanvasRenderer, WebCanvasRenderer};
    use web_sys::wasm_bindgen::JsCast;
    use web_sys::HtmlCanvasElement;

    let mut renderer: Signal<Option<WebCanvasRenderer>> = use_signal(|| None);
    let id = canvas_id.to_string();
    let mut initialized = use_signal(|| false);

    let id1 = id.clone();
    use_effect(move || {
        if !ready() || *initialized.read() {
            return;
        }
        let doc = web_sys::window().and_then(|w| w.document());
        if doc.is_none() {
            return;
        }
        let doc = doc.unwrap();
        if doc.get_element_by_id(&id1).is_none() {
            return;
        }
        let (w, h) = *size.read();
        let mut r = WebCanvasRenderer::new().with_canvas_id(&id1);
        r.resize(w, h);
        renderer.set(Some(r));
        *initialized.write() = true;
    });

    let id2 = id;
    use_effect(move || {
        let (w, h) = *size.read();
        if let Some(r) = renderer.write().as_mut() {
            r.resize(w, h);
        }
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(el) = doc.get_element_by_id(&id2) {
                if let Some(canvas) = el.dyn_ref::<HtmlCanvasElement>() {
                    canvas.set_width(w as u32);
                    canvas.set_height(h as u32);
                }
            }
        }
    });

    use_effect(move || {
        let state = render_state();
        if let Some(r) = renderer.write().as_mut() {
            r.render(&state);
        }
    });
}
