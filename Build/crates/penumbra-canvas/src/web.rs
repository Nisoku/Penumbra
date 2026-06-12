use std::collections::HashMap;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::renderer::GraphCanvasRenderer;
use crate::state::{Camera, RenderEdge, RenderState};

const GRID_SPACING: f64 = 28.0;

#[derive(Clone)]
pub struct WebCanvasRenderer {
    canvas: Option<HtmlCanvasElement>,
    ctx: Option<CanvasRenderingContext2d>,
    width: f32,
    height: f32,
    dot_color: String,
    edge_color: String,
}

impl WebCanvasRenderer {
    fn draw_grid(&self, ctx: &CanvasRenderingContext2d, camera: &Camera) {
        let inv = 1.0 / camera.zoom;
        let left = (-camera.x) * inv;
        let top = (-camera.y) * inv;
        let right = left + self.width as f64 * inv;
        let bottom = top + self.height as f64 * inv;

        let start_x = (left / GRID_SPACING).floor() * GRID_SPACING;
        let start_y = (top / GRID_SPACING).floor() * GRID_SPACING;

        ctx.set_fill_style(&self.dot_color.as_str().into());

        let mut gy = start_y;
        while gy <= bottom {
            let mut gx = start_x;
            while gx <= right {
                let sx = (gx + camera.x) * camera.zoom;
                let sy = (gy + camera.y) * camera.zoom;
                ctx.fill_rect(sx - 0.5, sy - 0.5, 1.0, 1.0);
                gx += GRID_SPACING;
            }
            gy += GRID_SPACING;
        }
    }

    fn draw_edge(
        &self,
        ctx: &CanvasRenderingContext2d,
        edge: &RenderEdge,
        camera: &Camera,
        node_map: &HashMap<penumbra_core::note::NoteId, (f64, f64)>,
    ) {
        let Some(&(sx, sy)) = node_map.get(&edge.source) else {
            return;
        };
        let Some(&(tx, ty)) = node_map.get(&edge.target) else {
            return;
        };

        let ax = (sx + camera.x) * camera.zoom;
        let ay = (sy + camera.y) * camera.zoom;
        let bx = (tx + camera.x) * camera.zoom;
        let by = (ty + camera.y) * camera.zoom;

        let cx = (ax + bx) / 2.0;

        ctx.begin_path();
        ctx.move_to(ax, ay);
        ctx.bezier_curve_to(cx, ay, cx, by, bx, by);
        ctx.set_stroke_style(&self.edge_color.as_str().into());
        ctx.set_global_alpha(edge.opacity as f64);
        let _ = ctx.stroke();
        ctx.set_global_alpha(1.0);
    }
}

impl GraphCanvasRenderer for WebCanvasRenderer {
    fn new() -> Self {
        Self {
            canvas: None,
            ctx: None,
            width: 800.0,
            height: 600.0,
            dot_color: "rgba(99, 148, 220, 0.18)".into(),
            edge_color: "rgba(99, 148, 220, 0.25)".into(),
        }
    }

    fn resize(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        if let Some(canvas) = &self.canvas {
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);
        }
    }

    fn render(&mut self, state: &RenderState) {
        let Some(ctx) = &self.ctx else { return };
        let w = self.width as f64;
        let h = self.height as f64;

        ctx.clear_rect(0.0, 0.0, w, h);

        self.draw_grid(ctx, &state.camera);

        let node_positions: HashMap<_, _> = state
            .nodes
            .iter()
            .map(|n| (n.id, (n.position.x, n.position.y)))
            .collect();

        for edge in &state.edges {
            self.draw_edge(ctx, edge, &state.camera, &node_positions);
        }
    }

    fn set_theme(&mut self, css_vars: &str) {
        tracing::debug!("canvas theme vars: {css_vars}");
    }
}

impl WebCanvasRenderer {
    pub fn with_canvas_id(mut self, id: &str) -> Self {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("no document available");
        let canvas = document
            .get_element_by_id(id)
            .and_then(|el| el.dyn_into::<HtmlCanvasElement>().ok())
            .expect("canvas element not found");
        let ctx = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok())
            .expect("failed to get 2d context");

        self.canvas = Some(canvas);
        self.ctx = Some(ctx);
        self
    }
}
