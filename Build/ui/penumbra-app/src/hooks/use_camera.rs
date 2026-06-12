use dioxus::prelude::*;
use penumbra_core::position::Position;

#[derive(Clone, Copy, PartialEq)]
pub struct CameraHandle {
    pub x: Signal<f64>,
    pub y: Signal<f64>,
    pub zoom: Signal<f64>,
    target_x: Signal<Option<f64>>,
    target_y: Signal<Option<f64>>,
    target_zoom: Signal<Option<f64>>,
}

impl CameraHandle {
    /// Whether the camera is currently drifting toward a target.
    pub fn is_drifting(&self) -> bool {
        self.target_x.read().is_some()
            || self.target_y.read().is_some()
            || self.target_zoom.read().is_some()
    }

    /// Apply a pan delta (screen-space pixels).
    pub fn pan(&mut self, dx: f64, dy: f64) {
        *self.x.write() += dx;
        *self.y.write() += dy;
        *self.target_x.write() = None;
        *self.target_y.write() = None;
    }

    /// Zoom by a factor at a given screen-space point.
    pub fn zoom_at(&mut self, dz: f64, cx: f64, cy: f64) {
        let old = *self.zoom.read();
        let new = (old * (1.0 + dz)).clamp(0.1, 5.0);
        let ratio = new / old;
        let cur_x = *self.x.read();
        let cur_y = *self.y.read();
        *self.x.write() = cx - (cx - cur_x) * ratio;
        *self.y.write() = cy - (cy - cur_y) * ratio;
        *self.zoom.write() = new;
        *self.target_zoom.write() = None;
    }

    /// Smoothly drift toward a world-space position.
    pub fn drift_to(&mut self, pos: Position) {
        *self.target_x.write() = Some(pos.x);
        *self.target_y.write() = Some(pos.y);
    }

    /// Smoothly zoom to a level.
    pub fn drift_zoom(&mut self, z: f64) {
        *self.target_zoom.write() = Some(z);
    }

    /// Cancel any in-flight drift.
    pub fn cancel_drift(&mut self) {
        *self.target_x.write() = None;
        *self.target_y.write() = None;
        *self.target_zoom.write() = None;
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, sx: f64, sy: f64) -> Position {
        let z = *self.zoom.read();
        Position::new((sx - *self.x.read()) / z, (sy - *self.y.read()) / z)
    }
}

/// Camera state with lerp-based drift toward a target.
pub fn use_camera() -> CameraHandle {
    let mut handle = CameraHandle {
        x: use_signal(|| 0.0),
        y: use_signal(|| 0.0),
        zoom: use_signal(|| 1.0),
        target_x: use_signal(|| None::<f64>),
        target_y: use_signal(|| None::<f64>),
        target_zoom: use_signal(|| None::<f64>),
    };

    use_effect(move || {
        let tx = *handle.target_x.read();
        let ty = *handle.target_y.read();
        let tz = *handle.target_zoom.read();
        if tx.is_none() && ty.is_none() && tz.is_none() {
            return;
        }

        let mut changed = false;

        if let Some(tx) = tx {
            let cur = *handle.x.read();
            let next = cur + (tx - cur) * 0.08;
            *handle.x.write() = next;
            if (tx - next).abs() < 0.5 {
                *handle.target_x.write() = None;
            }
            changed = true;
        }
        if let Some(ty) = ty {
            let cur = *handle.y.read();
            let next = cur + (ty - cur) * 0.08;
            *handle.y.write() = next;
            if (ty - next).abs() < 0.5 {
                *handle.target_y.write() = None;
            }
            changed = true;
        }
        if let Some(tz) = tz {
            let cur = *handle.zoom.read();
            let next = cur + (tz - cur) * 0.08;
            *handle.zoom.write() = next;
            if (tz - next).abs() < 0.01 {
                *handle.target_zoom.write() = None;
            }
            changed = true;
        }

        // Since we changed signals, the effect will re-run;
        // no explicit yield needed.
        let _ = changed;
    });

    handle
}
