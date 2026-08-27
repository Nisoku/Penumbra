use penumbra_core::position::Position;

use crate::LayoutConfig;

#[derive(Debug, Clone)]
pub struct CpuForceLayout {
    positions: Vec<Position>,
    velocities: Vec<Position>,
    masses: Vec<f64>,
    edges: Vec<(usize, usize)>,
    config: LayoutConfig,
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
        Self {
            positions,
            velocities: vec![Position::new(0.0, 0.0); n],
            masses: vec![1.0; n],
            edges,
            config,
        }
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
