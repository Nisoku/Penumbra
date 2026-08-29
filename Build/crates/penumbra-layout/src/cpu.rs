use std::collections::HashMap;

use penumbra_core::position::Position;

use crate::LayoutConfig;

#[derive(Debug, Clone)]
pub struct CpuForceLayout {
    positions: Vec<Position>,
    velocities: Vec<Position>,
    masses: Vec<f64>,
    edges: Vec<(usize, usize)>,
    config: LayoutConfig,
    /// Neighbor indices by source node for fast spring lookups.
    adjacency: Vec<Vec<usize>>,
}

/// Uniform-grid spatial index over node positions.
#[derive(Debug, Clone)]
pub(crate) struct SpatialHash {
    cell_size: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    positions: Vec<Position>,
}

impl SpatialHash {
    pub(crate) fn new(cell_size: f64, positions: Vec<Position>) -> Self {
        let mut hash = Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
            positions,
        };
        for idx in 0..hash.positions.len() {
            hash.insert(idx);
        }
        hash
    }

    fn cell_of(&self, pos: Position) -> (i64, i64) {
        (
            (pos.x / self.cell_size).floor() as i64,
            (pos.y / self.cell_size).floor() as i64,
        )
    }

    fn insert(&mut self, idx: usize) {
        let pos = self.positions[idx];
        self.cells.entry(self.cell_of(pos)).or_default().push(idx);
    }

    /// Return every node index within `radius` of `pos`.
    pub(crate) fn within(&self, pos: Position, radius: f64) -> Vec<usize> {
        let mut result = Vec::new();
        self.for_each_within(pos, radius, |idx| result.push(idx));
        result
    }

    /// Invoke `visit` for every node whose stored position is within `radius`
    /// of `pos`. Only cells overlapping the radius are examined.
    fn for_each_within(&self, pos: Position, radius: f64, mut visit: impl FnMut(usize)) {
        let spread = (radius / self.cell_size).ceil() as i64;
        let center = self.cell_of(pos);
        let radius_sq = radius * radius;
        for cx in (center.0 - spread)..=(center.0 + spread) {
            for cy in (center.1 - spread)..=(center.1 + spread) {
                let Some(indices) = self.cells.get(&(cx, cy)) else {
                    continue;
                };
                for &idx in indices {
                    if self.positions[idx].squared_distance_to(&pos) <= radius_sq {
                        visit(idx);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct QuadTree {
    center: Position,
    size: f64,
    mass: f64,
    center_of_mass: Position,
    count: usize,
    children: Option<[Box<QuadTree>; 4]>,
}

impl CpuForceLayout {
    pub fn new(positions: Vec<Position>, edges: Vec<(usize, usize)>, config: LayoutConfig) -> Self {
        let n = positions.len();
        let mut adjacency = vec![Vec::new(); n];
        for &(src, tgt) in &edges {
            if src < n && tgt < n {
                adjacency[src].push(tgt);
                adjacency[tgt].push(src);
            }
        }
        Self {
            positions,
            velocities: vec![Position::new(0.0, 0.0); n],
            masses: vec![1.0; n],
            edges,
            config,
            adjacency,
        }
    }

    /// Run one force-integration pass for the nodes within `radius` of `center`.
    pub fn step_neighborhood(&mut self, center: usize, radius: f64) -> Vec<usize> {
        if self.positions.is_empty() || self.positions.len() <= center {
            return Vec::new();
        }

        let hash = SpatialHash::new(radius * 2.0, self.positions.clone());
        let center_pos = self.positions[center];
        let neighborhood = hash.within(center_pos, radius);
        if neighborhood.is_empty() {
            return Vec::new();
        }

        let repulsion = self.config.repulsion as f64;
        let attraction = self.config.attraction as f64;
        let ideal_length = self.config.ideal_length as f64;
        let gravity = self.config.gravity as f64;
        let dt = self.config.dt as f64;
        let damping = self.config.damping as f64;

        let mut forces = vec![Position::new(0.0, 0.0); self.positions.len()];

        for &i in &neighborhood {
            let pi = self.positions[i];

            // Repulsion from every node within the repulsion radius of i; the
            // spatial hash bounds this to local cells.
            for j in hash.within(pi, radius) {
                if j == i {
                    continue;
                }
                let pj = self.positions[j];
                let dx = pi.x - pj.x;
                let dy = pi.y - pj.y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < 1.0 {
                    continue;
                }
                let dist = dist_sq.sqrt();
                let force = repulsion * self.masses[j] / dist_sq;
                forces[i].x += dx / dist * force;
                forces[i].y += dy / dist * force;
            }

            // Springs to all neighbors (inside or outside the neighborhood).
            for &j in &self.adjacency[i] {
                let pj = self.positions[j];
                let dx = pj.x - pi.x;
                let dy = pj.y - pi.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 1e-6 {
                    continue;
                }
                let displacement = dist - ideal_length;
                let force = attraction * displacement;
                let fx = dx / dist * force;
                let fy = dy / dist * force;
                // i feels the pull toward j; the neighborhood may not contain
                // j, in which case j stays put and only i moves.
                forces[i].x += fx;
                forces[i].y += fy;
            }

            forces[i].x -= pi.x * gravity * self.masses[i];
            forces[i].y -= pi.y * gravity * self.masses[i];
        }

        for &i in &neighborhood {
            let vel = &mut self.velocities[i];
            let f = forces[i];
            vel.x = (vel.x + f.x * dt) * damping;
            vel.y = (vel.y + f.y * dt) * damping;
            self.positions[i].x += vel.x * dt;
            self.positions[i].y += vel.y * dt;
        }

        neighborhood
    }

    /// Access the current positions (shared with the engine).
    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    pub fn step(&mut self) -> Vec<Position> {
        if self.positions.is_empty() {
            return self.positions.clone();
        }

        let mut forces = vec![Position::new(0.0, 0.0); self.positions.len()];

        let tree = self.build_quadtree();
        self.compute_repulsion(&tree, &mut forces);
        self.compute_spring_forces(&mut forces);
        self.compute_gravity(&mut forces);

        let dt = self.config.dt as f64;
        let damping = self.config.damping as f64;

        for (i, vel) in self.velocities.iter_mut().enumerate() {
            let f = forces[i];
            vel.x = (vel.x + f.x * dt) * damping;
            vel.y = (vel.y + f.y * dt) * damping;
            self.positions[i].x += vel.x * dt;
            self.positions[i].y += vel.y * dt;
        }

        self.positions.clone()
    }

    fn bounds(&self) -> (Position, Position) {
        let mut min = self.positions[0];
        let mut max = self.positions[0];
        for p in &self.positions[1..] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        (min, max)
    }

    fn build_quadtree(&self) -> QuadTree {
        let (min, max) = self.bounds();
        let size = (max.x - min.x).max(max.y - min.y).max(1.0);
        let center = Position::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);

        let mut tree = QuadTree {
            center,
            size,
            mass: 0.0,
            center_of_mass: Position::new(0.0, 0.0),
            count: 0,
            children: None,
        };

        for (i, pos) in self.positions.iter().enumerate() {
            tree.insert(*pos, self.masses[i]);
        }

        tree
    }

    fn compute_repulsion(&self, tree: &QuadTree, forces: &mut [Position]) {
        let repulsion = self.config.repulsion as f64;
        for (i, pos) in self.positions.iter().enumerate() {
            let mut fx = 0.0;
            let mut fy = 0.0;
            self.apply_repulsion(tree, *pos, repulsion, &mut fx, &mut fy);
            forces[i].x += fx;
            forces[i].y += fy;
        }
    }

    fn apply_repulsion(
        &self,
        node: &QuadTree,
        pos: Position,
        repulsion: f64,
        fx: &mut f64,
        fy: &mut f64,
    ) {
        if node.count == 0 {
            return;
        }

        if node.count == 1 && node.children.is_none() {
            let dx = pos.x - node.center_of_mass.x;
            let dy = pos.y - node.center_of_mass.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < 1.0 {
                return;
            }
            let dist = dist_sq.sqrt();
            let force = repulsion / dist_sq;
            *fx += dx / dist * force;
            *fy += dy / dist * force;
            return;
        }

        let dx = pos.x - node.center_of_mass.x;
        let dy = pos.y - node.center_of_mass.y;
        let dist_sq = dx * dx + dy * dy;
        let ratio = node.size * node.size;
        let theta = self.config.theta as f64;

        if ratio < dist_sq * theta * theta {
            if dist_sq < 1.0 {
                return;
            }
            let dist = dist_sq.sqrt();
            let force = repulsion * node.mass / dist_sq;
            *fx += dx / dist * force;
            *fy += dy / dist * force;
        } else if let Some(ref children) = node.children {
            for child in children {
                self.apply_repulsion(child, pos, repulsion, fx, fy);
            }
        }
    }

    fn compute_spring_forces(&self, forces: &mut [Position]) {
        let attraction = self.config.attraction as f64;
        let ideal_length = self.config.ideal_length as f64;

        for &(src, tgt) in &self.edges {
            let dx = self.positions[tgt].x - self.positions[src].x;
            let dy = self.positions[tgt].y - self.positions[src].y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 1e-6 {
                continue;
            }
            let displacement = dist - ideal_length;
            let force = attraction * displacement;
            let fx = dx / dist * force;
            let fy = dy / dist * force;
            forces[src].x += fx;
            forces[src].y += fy;
            forces[tgt].x -= fx;
            forces[tgt].y -= fy;
        }
    }

    fn compute_gravity(&self, forces: &mut [Position]) {
        let gravity = self.config.gravity as f64;
        for (i, pos) in self.positions.iter().enumerate() {
            forces[i].x -= pos.x * gravity * self.masses[i];
            forces[i].y -= pos.y * gravity * self.masses[i];
        }
    }
}

impl QuadTree {
    fn insert(&mut self, pos: Position, mass: f64) {
        if self.count == 0 {
            self.mass = mass;
            self.center_of_mass = pos;
            self.count = 1;
            return;
        }

        if self.children.is_none() {
            self.subdivide();
        }

        let quadrant = self.quadrant(&pos);
        if let Some(ref mut children) = self.children {
            children[quadrant].insert(pos, mass);
        }

        let total_mass = self.mass + mass;
        self.center_of_mass.x = (self.center_of_mass.x * self.mass + pos.x * mass) / total_mass;
        self.center_of_mass.y = (self.center_of_mass.y * self.mass + pos.y * mass) / total_mass;
        self.mass = total_mass;
        self.count += 1;
    }

    fn subdivide(&mut self) {
        let half = self.size * 0.5;
        let quarter = self.size * 0.25;
        let c = self.center;

        self.children = Some([
            Box::new(QuadTree::leaf(
                Position::new(c.x - quarter, c.y - quarter),
                half,
            )),
            Box::new(QuadTree::leaf(
                Position::new(c.x + quarter, c.y - quarter),
                half,
            )),
            Box::new(QuadTree::leaf(
                Position::new(c.x - quarter, c.y + quarter),
                half,
            )),
            Box::new(QuadTree::leaf(
                Position::new(c.x + quarter, c.y + quarter),
                half,
            )),
        ]);
    }

    fn leaf(center: Position, size: f64) -> Self {
        Self {
            center,
            size,
            mass: 0.0,
            center_of_mass: Position::new(0.0, 0.0),
            count: 0,
            children: None,
        }
    }

    fn quadrant(&self, pos: &Position) -> usize {
        let mut q = 0;
        if pos.x >= self.center.x {
            q += 1;
        }
        if pos.y >= self.center.y {
            q += 2;
        }
        q
    }
}
