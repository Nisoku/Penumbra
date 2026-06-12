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

    // Inject the RAF drawing loop once on ready
    let inject_id = id.clone();
    use_effect(move || {
        if !ready() || *initted.read() {
            return;
        }
        *initted.write() = true;
        let js = format!(
            r#"(function(){{
const canvas=document.getElementById('{inject_id}');
if(!canvas)return;
(function draw(){{
const state=window.__penumbra_state;
const w=canvas.width,h=canvas.height;
const ctx=canvas.getContext('2d');
if(!ctx){{requestAnimationFrame(draw);return;}}
ctx.clearRect(0,0,w,h);
if(state&&state.camera){{
ctx.save();
ctx.translate(state.camera.x,state.camera.y);
ctx.scale(state.camera.zoom,state.camera.zoom);
if(state.edges){{
for(const e of state.edges){{
const src=state.nodes.find(n=>n.id===e.source);
const tgt=state.nodes.find(n=>n.id===e.target);
if(!src||!tgt)continue;
ctx.beginPath();
ctx.moveTo(src.position.x,src.position.y);
ctx.lineTo(tgt.position.x,tgt.position.y);
ctx.strokeStyle=e.opacity>0.5?'rgba(99,148,220,0.7)':'rgba(99,148,220,0.35)';
ctx.lineWidth=1.5;
ctx.stroke();
}}
}}
if(state.nodes){{
for(const n of state.nodes){{
const x=n.position.x;
const y=n.position.y;
const sel=n.id===state.selected_node;
ctx.beginPath();
ctx.arc(x,y,sel?6:4,0,Math.PI*2);
ctx.fillStyle=sel?'#6394dc':'#2a4a7a';
ctx.fill();
}}
}}
ctx.restore();
}}
requestAnimationFrame(draw);
}})();
}})();"#
        );
        _ = document::eval(&js);
    });

    // Resize canvas when window size changes
    let resize_id = id.clone();
    use_effect(move || {
        if !ready() {
            return;
        }
        let (w, h) = *size.read();
        _ = document::eval(&format!(
            "const c=document.getElementById('{resize_id}');if(c){{c.width={w};c.height={h};}}"
        ));
    });

    // Push render state to JS on every change
    use_effect(move || {
        if !ready() {
            return;
        }
        let state = render_state();
        match serde_json::to_string(&state) {
            Ok(json) => {
                _ = document::eval(&format!("window.__penumbra_state={json};"));
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
