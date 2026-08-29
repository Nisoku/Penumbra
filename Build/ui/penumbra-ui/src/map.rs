use crate::NoteCardVM;

pub const GRID_SPACING: f32 = 120.0;
const VIEWPORT_MARGIN: f32 = 40.0;
const CURVE_BEND: f32 = 0.18;
const MIN_BEND_PX: f32 = 8.0;
const MAX_BEND_PX: f32 = 90.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct MapCamera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Clone, Debug, Default)]
pub struct MapEdges {
    pub base: String,
    pub selected: String,
}

pub fn build_edges(
    cards: &[NoteCardVM],
    links: &[(i32, i32)],
    cam: MapCamera,
    view_w: f32,
    view_h: f32,
    selected: i32,
) -> MapEdges {
    let positions: std::collections::HashMap<i32, (f32, f32)> = cards
        .iter()
        .map(|card| (card.id, (card.x, card.y)))
        .collect();

    let mut base = String::new();
    let mut selected_path = String::new();
    for (a, b) in links {
        let Some(&(wx, wy)) = positions.get(a) else {
            continue;
        };
        let Some(&(ux, uy)) = positions.get(b) else {
            continue;
        };
        let (sx1, sy1) = ((wx - cam.x) * cam.zoom, (wy - cam.y) * cam.zoom);
        let (sx2, sy2) = ((ux - cam.x) * cam.zoom, (uy - cam.y) * cam.zoom);
        let len = ((sx2 - sx1).powi(2) + (sy2 - sy1).powi(2)).sqrt();
        if len < 1.0
            || !is_visible(sx1, sy1, view_w, view_h) && !is_visible(sx2, sy2, view_w, view_h)
        {
            continue;
        }

        let bend = (len * CURVE_BEND).clamp(MIN_BEND_PX, MAX_BEND_PX);
        let (mx, my) = ((sx1 + sx2) * 0.5, (sy1 + sy2) * 0.5);
        let inv = 1.0 / len;
        let (nx, ny) = (-(sy2 - sy1) * inv, (sx2 - sx1) * inv);
        let (cx, cy) = (mx + nx * bend, my + ny * bend);
        let seg =
            format!("M {sx1:.1} {sy1:.1} C {cx:.1} {cy:.1} {cx:.1} {cy:.1} {sx2:.1} {sy2:.1} ");
        if *a == selected || *b == selected {
            selected_path.push_str(&seg);
        } else {
            base.push_str(&seg);
        }
    }
    MapEdges {
        base,
        selected: selected_path,
    }
}

pub fn build_grid(cam: MapCamera, view_w: f32, view_h: f32, spacing: f32) -> String {
    let min_gap = 24.0;
    let screen_gap = spacing * cam.zoom;
    let step = if screen_gap < min_gap {
        (min_gap / screen_gap).ceil() * spacing
    } else {
        spacing
    };

    let mut out = String::new();
    let first_col = (cam.x / step).floor() * step;
    let col_count = (view_w / cam.zoom / step).ceil() as usize + 1;
    for k in 0..col_count {
        let wx = first_col + k as f32 * step;
        let sx = (wx - cam.x) * cam.zoom;
        out.push_str(&format!("M {sx:.1} 0 L {sx:.1} {view_h:.1} "));
    }
    let first_row = (cam.y / step).floor() * step;
    let row_count = (view_h / cam.zoom / step).ceil() as usize + 1;
    for k in 0..row_count {
        let wy = first_row + k as f32 * step;
        let sy = (wy - cam.y) * cam.zoom;
        out.push_str(&format!("M 0 {sy:.1} L {view_w:.1} {sy:.1} "));
    }
    out
}

fn is_visible(sx: f32, sy: f32, view_w: f32, view_h: f32) -> bool {
    let m = VIEWPORT_MARGIN;
    sx >= -m && sx <= view_w + m && sy >= -m && sy <= view_h + m
}
