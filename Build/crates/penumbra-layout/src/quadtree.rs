use std::collections::HashMap;

use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

use crate::{ForceAccumulator, NodeState};

/// Below this cell half-width, nodes are grouped together rather than
/// subdividing further. This caps recursion depth to prevent stack overflow
/// when many nodes occupy nearly the same coordinate.
const MIN_CELL_HALF: f64 = 0.01;

/// A quadtree for the Barnes-Hut approximation of n-body repulsion.
///
/// Distant groups of nodes are approximated as a single "supernode" at their
/// center of mass, reducing the O(n²) all-pairs repulsion to O(n log n).
#[derive(Debug, Clone)]
pub struct BarnesHutTree {
    root: Option<Box<Quadrant>>,
}

#[derive(Debug, Clone)]
struct Quadrant {
    cx: f64,
    cy: f64,
    half: f64,
    mass_cx: f64,
    mass_cy: f64,
    total_mass: usize,
    single_id: Option<NoteId>,
    children: Option<[Box<Quadrant>; 4]>,
}

impl BarnesHutTree {
    pub(crate) fn build(nodes: &HashMap<NoteId, NodeState>) -> Self {
        if nodes.is_empty() {
            return Self { root: None };
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for state in nodes.values() {
            let x = state.position.x;
            let y = state.position.y;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        let eps = 1.0;
        min_x -= eps;
        min_y -= eps;
        max_x += eps;
        max_y += eps;

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let half = ((max_x - min_x).max(max_y - min_y) / 2.0).max(1.0);

        let mut root = Box::new(Quadrant {
            cx,
            cy,
            half,
            mass_cx: 0.0,
            mass_cy: 0.0,
            total_mass: 0,
            single_id: None,
            children: None,
        });

        for (id, state) in nodes {
            root.insert(*id, state.position.x, state.position.y);
        }

        Self { root: Some(root) }
    }

    pub(crate) fn apply_repulsive(
        &self,
        node_id: &NoteId,
        pos: &Position,
        theta: f64,
        scaling: f64,
        acc: &mut ForceAccumulator,
    ) {
        let Some(ref root) = self.root else {
            return;
        };
        root.apply_repulsive(node_id, pos.x, pos.y, theta, scaling, acc);
    }
}

impl Quadrant {
    fn insert(&mut self, id: NoteId, x: f64, y: f64) {
        let total = self.total_mass;
        self.mass_cx = (self.mass_cx * total as f64 + x) / (total + 1) as f64;
        self.mass_cy = (self.mass_cy * total as f64 + y) / (total + 1) as f64;
        self.total_mass += 1;

        if self.total_mass == 1 {
            self.single_id = Some(id);
            return;
        }

        if self.children.is_none() && self.half > MIN_CELL_HALF {
            let h = self.half / 2.0;
            let offsets = [
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (1.0, 1.0),
            ];
            let mut children: [Box<Quadrant>; 4] = offsets.map(|(dx, dy)| {
                Box::new(Quadrant {
                    cx: self.cx + dx * h,
                    cy: self.cy + dy * h,
                    half: h,
                    mass_cx: 0.0,
                    mass_cy: 0.0,
                    total_mass: 0,
                    single_id: None,
                    children: None,
                })
            });

            if let Some(prev_id) = self.single_id {
                let prev_x = self.mass_cx;
                let prev_y = self.mass_cy;
                let child_idx = child_index(prev_x, prev_y, self.cx, self.cy);
                children[child_idx].insert(prev_id, prev_x, prev_y);
                self.single_id = None;
            }

            self.children = Some(children);
        }

        if let Some(ref mut children) = self.children {
            let child_idx = child_index(x, y, self.cx, self.cy);
            children[child_idx].insert(id, x, y);
        }
    }

    fn apply_repulsive(
        &self,
        node_id: &NoteId,
        x: f64,
        y: f64,
        theta: f64,
        scaling: f64,
        acc: &mut ForceAccumulator,
    ) {
        if self.total_mass == 0 {
            return;
        }

        let dx = self.mass_cx - x;
        let dy = self.mass_cy - y;
        let dist_sq = dx * dx + dy * dy;
        let dist = dist_sq.sqrt().max(1.0);

        if self.total_mass == 1 {
            if let Some(id) = self.single_id {
                if id == *node_id {
                    return;
                }
            }
            let force = scaling / dist_sq;
            acc.fx += force * dx / dist;
            acc.fy += force * dy / dist;
            return;
        }

        let cell_size = self.half * 2.0;
        if cell_size / dist < theta {
            let force = scaling * self.total_mass as f64 / dist_sq;
            acc.fx += force * dx / dist;
            acc.fy += force * dy / dist;
            acc.approximation_count += 1;
        } else if let Some(ref children) = self.children {
            for child in children.iter() {
                child.apply_repulsive(node_id, x, y, theta, scaling, acc);
            }
        }
    }
}

fn child_index(x: f64, y: f64, cx: f64, cy: f64) -> usize {
    let left = x < cx;
    let top = y < cy;
    match (left, top) {
        (true, true) => 0,
        (false, true) => 1,
        (true, false) => 2,
        (false, false) => 3,
    }
}
