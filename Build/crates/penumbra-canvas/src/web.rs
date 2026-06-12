use std::collections::HashMap;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::renderer::GraphCanvasRenderer;
use crate::state::{Camera, RenderEdge, RenderNode, RenderState};

const NODE_WIDTH: f64 = 180.0;
const NODE_HEIGHT: f64 = 60.0;
const NODE_RADIUS: f64 = 10.0;
const GRID_SPACING: f64 = 28.0;

fn round_rect(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    ctx.arc_to(x + w, y, x + w, y + r, r);
    ctx.line_to(x + w, y + h - r);
    ctx.arc_to(x + w, y + h, x + w - r, y + h, r);
    ctx.line_to(x + r, y + h);
    ctx.arc_to(x, y + h, x, y + h - r, r);
    ctx.line_to(x, y + r);
    ctx.arc_to(x, y, x + r, y, r);
    ctx.close_path();
}

#[derive(Clone)]
pub struct WebCanvasRenderer {
    canvas: Option<HtmlCanvasElement>,
    ctx: Option<CanvasRenderingContext2d>,
    width: f32,
    height: f32,
    dot_color: String,
    edge_color: String,
    node_bg: String,
    node_border: String,
    node_text: String,
    node_selected_border: String,
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
        let cy = (ay + by) / 2.0;

        ctx.begin_path();
        ctx.move_to(ax, ay);
        ctx.bezier_curve_to(cx, ay, cx, by, bx, by);
        ctx.set_stroke_style(&self.edge_color.as_str().into());
        ctx.set_global_alpha(edge.opacity as f64);
        let _ = ctx.stroke();
        ctx.set_global_alpha(1.0);
    }

    fn draw_node(
        &self,
        ctx: &CanvasRenderingContext2d,
        node: &RenderNode,
        camera: &Camera,
        is_selected: bool,
    ) {
        let x = (node.position.x + camera.x) * camera.zoom;
        let y = (node.position.y + camera.y) * camera.zoom;
        let w = NODE_WIDTH * camera.zoom;
        let h = NODE_HEIGHT * camera.zoom;
        let r = NODE_RADIUS * camera.zoom.min(1.0);

        round_rect(ctx, x, y, w, h, r);

        ctx.set_fill_style(&self.node_bg.as_str().into());
        let _ = ctx.fill();

        ctx.set_line_width(if is_selected { 2.0 } else { 1.0 });
        ctx.set_stroke_style(
            &(if is_selected {
                &self.node_selected_border
            } else {
                &self.node_border
            })
            .as_str()
            .into(),
        );
        let _ = ctx.stroke();

        let fs = (12.0 * camera.zoom).max(6.0);
        ctx.set_font(&format!("{}px sans-serif", fs));
        ctx.set_fill_style(&self.node_text.as_str().into());
        let _ = ctx.fill_text(&node.title, x + 8.0 * camera.zoom, y + h / 2.0 + fs * 0.35);
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
            node_bg: "rgba(15, 30, 60, 0.85)".into(),
            node_border: "rgba(99, 148, 220, 0.2)".into(),
            node_text: "#c8e3ff".into(),
            node_selected_border: "#7abaff".into(),
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

        for node in &state.nodes {
            let is_selected = state.selected_node == Some(node.id);
            self.draw_node(ctx, node, &state.camera, is_selected);
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
