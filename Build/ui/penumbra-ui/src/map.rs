use crate::NoteCardVM;

pub const GRID_SPACING: f32 = 120.0;
pub const CARD_READING_ZOOM: f32 = 1.8;
pub const CARD_W: f32 = 180.0;
pub const CARD_H: f32 = 120.0;
const MAX_ZOOM: f32 = 8.0;
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
        .map(|card| (card.id, (card.x + CARD_W * 0.5, card.y + CARD_H * 0.5)))
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

/// Screen-space path for an in-progress manual link
pub fn build_pending_link(
    source_x: f32,
    source_y: f32,
    pointer_wx: f32,
    pointer_wy: f32,
    cam: MapCamera,
) -> String {
    let sx = (source_x + CARD_W * 0.5 - cam.x) * cam.zoom;
    let sy = (source_y + CARD_H * 0.5 - cam.y) * cam.zoom;
    let px = (pointer_wx - cam.x) * cam.zoom;
    let py = (pointer_wy - cam.y) * cam.zoom;
    format!("M {sx:.1} {sy:.1} L {px:.1} {py:.1} ")
}

/// Which card (by id) contains the given world point, if any. Cards are matched
/// against their fixed on-screen footprint anchored at `(card.x, card.y)`.
pub fn card_at_world(cards: &[NoteCardVM], wx: f32, wy: f32) -> Option<i32> {
    cards
        .iter()
        .find(|card| wx >= card.x && wx <= card.x + CARD_W && wy >= card.y && wy <= card.y + CARD_H)
        .map(|card| card.id)
}

/// The camera that frames a single card at reading zoom.
pub fn frame_card(card_x: f32, card_y: f32, view_w: f32, view_h: f32) -> MapCamera {
    MapCamera {
        x: card_x - view_w / CARD_READING_ZOOM * 0.5,
        y: card_y - view_h / CARD_READING_ZOOM * 0.38,
        zoom: CARD_READING_ZOOM,
    }
}

pub fn zoom_card(card_x: f32, card_y: f32, view_w: f32, view_h: f32) -> MapCamera {
    let zoom = (view_w / CARD_W).clamp(CARD_READING_ZOOM, MAX_ZOOM);
    MapCamera {
        x: card_x - view_w / zoom * 0.5,
        y: card_y - view_h / zoom * 0.5,
        zoom,
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
