use std::collections::{HashMap, HashSet};

use penumbra_core::link::Link;
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;

pub mod quadtree;

use quadtree::BarnesHutTree;

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Global scaling for repulsive forces.
    pub scaling_ratio: f64,
    /// Gravitational constant pulling nodes toward center.
    pub gravity: f64,
    /// Barnes-Hut approximation threshold (higher = faster but less accurate).
    pub barnes_hut_theta: f64,
    pub max_iterations: usize,
    /// Average displacement below which layout is considered converged.
    pub convergence_threshold: f64,
    pub initial_speed: f64,
    pub max_speed: f64,
    pub tolerance: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            scaling_ratio: 2.0,
            gravity: 1.0,
            barnes_hut_theta: 1.2,
            max_iterations: 200,
            convergence_threshold: 0.1,
            initial_speed: 1.0,
            max_speed: 10.0,
            tolerance: 0.1,
        }
    }
}

/// Tracks force accumulation for a single node during one layout step.
#[derive(Debug, Clone, Default)]
struct ForceAccumulator {
    fx: f64,
    fy: f64,
    /// Number of Barnes-Hut approximations folded into this accumulator
    /// (used for adaptive scaling).
    approximation_count: usize,
}

#[derive(Debug, Clone)]
struct NodeState {
    position: Position,
    velocity: (f64, f64),
    pinned: bool,
}

pub struct LayoutEngine {
    nodes: HashMap<NoteId, NodeState>,
    links: Vec<Link>,
    config: LayoutConfig,
    iteration: usize,
}

impl LayoutEngine {
    pub fn new(config: LayoutConfig) -> Self {
        Self {
            nodes: HashMap::new(),
            links: Vec::new(),
            config,
            iteration: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(LayoutConfig::default())
    }

    pub fn add_node(&mut self, id: NoteId, pinned: bool) {
        let pos = Self::random_position();
        self.nodes.insert(
            id,
            NodeState {
                position: pos,
                velocity: (0.0, 0.0),
                pinned,
            },
        );
    }

    pub fn remove_node(&mut self, id: &NoteId) {
        self.nodes.remove(id);
        self.links.retain(|l| l.source != *id && l.target != *id);
    }

    pub fn update_links(&mut self, links: Vec<Link>) {
        self.links = links;
    }

    pub fn set_position(&mut self, id: &NoteId, pos: Position) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.position = pos;
        }
    }

    pub fn pin(&mut self, id: &NoteId, pinned: bool) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.pinned = pinned;
        }
    }

    pub fn get_position(&self, id: &NoteId) -> Option<Position> {
        self.nodes.get(id).map(|n| n.position)
    }

    pub fn all_positions(&self) -> HashMap<NoteId, Position> {
        self.nodes
            .iter()
            .map(|(id, state)| (*id, state.position))
            .collect()
    }

    pub fn is_converged(&self) -> bool {
        self.iteration >= self.config.max_iterations
    }

    pub fn iteration_count(&self) -> usize {
        self.iteration
    }

    /// Run a single iteration of the ForceAtlas2 algorithm.
    ///
    /// Returns the average displacement for convergence checking.
    pub fn step(&mut self) -> f64 {
        if self.nodes.is_empty() {
            self.iteration += 1;
            return 0.0;
        }

        let n = self.nodes.len();
        let mut accumulators: HashMap<NoteId, ForceAccumulator> = self
            .nodes
            .keys()
            .map(|id| (*id, ForceAccumulator::default()))
            .collect();

        let tree = BarnesHutTree::build(&self.nodes);

        // Repulsive forces: Barnes-Hut approximation
        let theta = self.config.barnes_hut_theta;
        for (id, state) in &self.nodes {
            if state.pinned {
                continue;
            }
            let acc = accumulators.get_mut(id).unwrap();
            tree.apply_repulsive(id, &state.position, theta, self.config.scaling_ratio, acc);
        }

        // Attractive forces: along edges
        for link in &self.links {
            let Some(source_state) = self.nodes.get(&link.source) else {
                continue;
            };
            let Some(target_state) = self.nodes.get(&link.target) else {
                continue;
            };

            let dx = target_state.position.x - source_state.position.x;
            let dy = target_state.position.y - source_state.position.y;
            let dist_sq = dx * dx + dy * dy;
            let dist = dist_sq.sqrt().max(1.0);

            let force = link.weight * dist;

            if let Some(acc) = accumulators.get_mut(&link.source) {
                if !source_state.pinned {
                    acc.fx += force * dx / dist;
                    acc.fy += force * dy / dist;
                }
            }
            if let Some(acc) = accumulators.get_mut(&link.target) {
                if !target_state.pinned {
                    acc.fx -= force * dx / dist;
                    acc.fy -= force * dy / dist;
                }
            }
        }

        // Gravity
        let gravity = self.config.gravity;
        for (id, state) in &self.nodes {
            if state.pinned {
                continue;
            }
            let dist = (state.position.x * state.position.x
                + state.position.y * state.position.y)
                .sqrt()
                .max(1.0);
            let acc = accumulators.get_mut(id).unwrap();
            acc.fx -= gravity * state.position.x / dist;
            acc.fy -= gravity * state.position.y / dist;
        }

        // Apply forces with adaptive speed
        let speed = self.adaptive_speed(&accumulators);
        let mut total_displacement = 0.0;

        for (id, state) in self.nodes.iter_mut() {
            if state.pinned {
                continue;
            }
            let acc = &accumulators[id];

            let delta_x = acc.fx * speed;
            let delta_y = acc.fy * speed;

            // Clamp displacement
            let delta_dist = (delta_x * delta_x + delta_y * delta_y).sqrt();
            let max_dist = self.config.max_speed * speed;
            let (clamped_dx, clamped_dy) = if delta_dist > max_dist {
                let scale = max_dist / delta_dist;
                (delta_x * scale, delta_y * scale)
            } else {
                (delta_x, delta_y)
            };

            state.position.x += clamped_dx;
            state.position.y += clamped_dy;

            state.velocity = (clamped_dx / speed, clamped_dy / speed);

            total_displacement += (clamped_dx * clamped_dx + clamped_dy * clamped_dy).sqrt();
        }

        self.iteration += 1;
        total_displacement / n as f64
    }

    /// Run the layout to convergence or max iterations.
    ///
    /// Returns the number of iterations actually executed.
    pub fn run(&mut self) -> usize {
        let start = self.iteration;
        let max = start + self.config.max_iterations;
        while self.iteration < max {
            let displacement = self.step();
            if displacement < self.config.convergence_threshold {
                break;
            }
        }
        self.iteration - start
    }

    fn adaptive_speed(&self, accumulators: &HashMap<NoteId, ForceAccumulator>) -> f64 {
        let mut total_force = 0.0f64;
        let mut count = 0.0f64;
        for acc in accumulators.values() {
            let force = (acc.fx * acc.fx + acc.fy * acc.fy).sqrt();
            total_force += force;
            count += 1.0;
        }
        let avg_force = total_force / f64::max(count, 1.0);
        let speed = (self.config.initial_speed * self.config.tolerance) / avg_force.max(0.001);
        speed.clamp(0.01, self.config.max_speed)
    }

    fn random_position() -> Position {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let seed = nanos as f64;
        let angle = seed * std::f64::consts::TAU / 1_000_000_000.0;
        let radius = 50.0 + (seed % 200.0);
        Position::new(angle.cos() * radius, angle.sin() * radius)
    }

    /// Incremental update: only recalculate forces for a specific node
    /// and its neighbors. Other nodes keep their positions.
    pub fn step_neighborhood(&mut self, id: &NoteId) -> f64 {
        let neighbor_ids: HashSet<NoteId> = self
            .links
            .iter()
            .filter_map(|l| {
                if l.source == *id {
                    Some(l.target)
                } else if l.target == *id {
                    Some(l.source)
                } else {
                    None
                }
            })
            .collect();

        let mut affected = neighbor_ids.clone();
        affected.insert(*id);

        // Full step for the whole graph is still needed for Barnes-Hut accuracy,
        // but we only update positions of affected nodes.
        if self.nodes.is_empty() {
            return 0.0;
        }

        let tree = BarnesHutTree::build(&self.nodes);

        let mut accumulators: HashMap<NoteId, ForceAccumulator> = self
            .nodes
            .keys()
            .map(|id| (*id, ForceAccumulator::default()))
            .collect();

        let theta = self.config.barnes_hut_theta;

        for nid in &affected {
            let Some(state) = self.nodes.get(nid) else {
                continue;
            };
            if state.pinned {
                continue;
            }
            let acc = accumulators.get_mut(nid).unwrap();
            tree.apply_repulsive(nid, &state.position, theta, self.config.scaling_ratio, acc);
        }

        for link in &self.links {
            if !affected.contains(&link.source) && !affected.contains(&link.target) {
                continue;
            }
            let Some(source_state) = self.nodes.get(&link.source) else {
                continue;
            };
            let Some(target_state) = self.nodes.get(&link.target) else {
                continue;
            };
            let dx = target_state.position.x - source_state.position.x;
            let dy = target_state.position.y - source_state.position.y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let force = link.weight * dist;

            if affected.contains(&link.source) {
                if let Some(acc) = accumulators.get_mut(&link.source) {
                    if !source_state.pinned {
                        acc.fx += force * dx / dist;
                        acc.fy += force * dy / dist;
                    }
                }
            }
            if affected.contains(&link.target) {
                if let Some(acc) = accumulators.get_mut(&link.target) {
                    if !target_state.pinned {
                        acc.fx -= force * dx / dist;
                        acc.fy -= force * dy / dist;
                    }
                }
            }
        }

        let gravity = self.config.gravity;
        for nid in &affected {
            let Some(state) = self.nodes.get(nid) else {
                continue;
            };
            if state.pinned {
                continue;
            }
            let dist = (state.position.x * state.position.x
                + state.position.y * state.position.y)
                .sqrt()
                .max(1.0);
            let acc = accumulators.get_mut(nid).unwrap();
            acc.fx -= gravity * state.position.x / dist;
            acc.fy -= gravity * state.position.y / dist;
        }

        let speed = self.adaptive_speed(&accumulators);
        let mut total_displacement = 0.0;

        for nid in &affected {
            let Some(state) = self.nodes.get_mut(nid) else {
                continue;
            };
            if state.pinned {
                continue;
            }
            let acc = &accumulators[nid];
            let delta_x = acc.fx * speed;
            let delta_y = acc.fy * speed;
            let delta_dist = (delta_x * delta_x + delta_y * delta_y).sqrt();
            let max_d = self.config.max_speed * speed;
            let (cdx, cdy) = if delta_dist > max_d {
                let s = max_d / delta_dist;
                (delta_x * s, delta_y * s)
            } else {
                (delta_x, delta_y)
            };
            state.position.x += cdx;
            state.position.y += cdy;
            state.velocity = (cdx / speed, cdy / speed);
            total_displacement += (cdx * cdx + cdy * cdy).sqrt();
        }

        total_displacement / affected.len().max(1) as f64
    }
}
