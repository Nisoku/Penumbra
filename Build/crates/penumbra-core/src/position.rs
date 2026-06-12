use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn squared_distance_to(&self, other: &Position) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

impl std::ops::Add for Position {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Position {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f64> for Position {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl From<(f64, f64)> for Position {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub const fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn contains(&self, pos: &Position, point: &Position) -> bool {
        point.x >= pos.x
            && point.x <= pos.x + self.width
            && point.y >= pos.y
            && point.y <= pos.y + self.height
    }

    pub fn overlaps(&self, pos_a: &Position, bounds_b: &Bounds, pos_b: &Position) -> bool {
        let a_left = pos_a.x;
        let a_right = pos_a.x + self.width;
        let a_top = pos_a.y;
        let a_bottom = pos_a.y + self.height;

        let b_left = pos_b.x;
        let b_right = pos_b.x + bounds_b.width;
        let b_top = pos_b.y;
        let b_bottom = pos_b.y + bounds_b.height;

        a_left < b_right && a_right > b_left && a_top < b_bottom && a_bottom > b_top
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 150.0,
        }
    }
}
