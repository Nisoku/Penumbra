mod cpu;

use std::collections::HashMap;

use penumbra_core::link::Link;
use penumbra_core::note::NoteId;
use penumbra_core::position::{Bounds, Position};
use vibe_graph_layout_gpu::{
    Edge as VibeEdge, GpuLayout, LayoutConfig as GpuConfig, Position as VibePos,
};

use cpu::{CpuForceLayout, SpatialHash};

/// Configuration for the GPU-accelerated layout engine.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub dt: f32,
    pub damping: f32,
    pub repulsion: f32,
    pub attraction: f32,
    pub theta: f32,
    pub gravity: f32,
    pub ideal_length: f32,
    pub max_iterations: usize,
    pub convergence_threshold: f64,
    /// Minimum gap between node bounding boxes after collision avoidance.
    pub collision_margin: f64,
    /// Maximum displacement per collision resolution pass (pixels).
    pub max_collision_push: f64,
    /// Number of iterative passes for collision resolution.
    pub collision_passes: usize,
    /// Geographic radius (pixels) of a neighborhood for `step_neighborhood`.
    pub neighborhood_radius: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            dt: 0.016,
            damping: 0.9,
            repulsion: 1000.0,
            attraction: 0.01,
            theta: 0.8,
            gravity: 0.1,
            ideal_length: 50.0,
            max_iterations: 200,
            convergence_threshold: 0.1,
            collision_margin: 10.0,
            max_collision_push: 20.0,
            collision_passes: 5,
            neighborhood_radius: 400.0,
        }
    }
}

impl From<&LayoutConfig> for GpuConfig {
    fn from(c: &LayoutConfig) -> Self {
        Self {
            dt: c.dt,
            damping: c.damping,
            repulsion: c.repulsion,
            attraction: c.attraction,
            theta: c.theta,
            gravity: c.gravity,
            ideal_length: c.ideal_length,
            use_barnes_hut: true,
            max_tree_depth: 12,
        }
    }
}

/// GPU-accelerated force-directed graph layout engine.
///
/// Wraps `vibe-graph-layout-gpu::GpuLayout` and maps between Penumbra's domain
/// types (NoteId, f64 positions, Links) and the GPU crate's types (u32 indices,
/// f32 positions, Edges). GPU initialisation happens in [`new`] so it must be
/// awaited. If GPU creation fails the engine still exists but `step()` returns
/// zero displacement (graceful degradation for headless / WebGPU-less enviros).
pub struct LayoutEngine {
    inner: Option<GpuLayout>,
    cpu: Option<CpuForceLayout>,
    nodes: Vec<NoteId>,
    index_map: HashMap<NoteId, u32>,
    positions: Vec<VibePos>,
    edges: Vec<VibeEdge>,
    pinned: HashMap<NoteId, VibePos>,
    node_bounds: HashMap<NoteId, Bounds>,
    config: LayoutConfig,
    iteration: usize,
    dirty: bool,
}

impl LayoutEngine {
    /// Create a new layout engine, initialising the GPU backend immediately.
    ///
    /// Returns `LayoutEngine` even if GPU creation fails so `step()` gracefully
    /// returns zero displacement when no GPU is available.
    pub async fn new(config: LayoutConfig) -> Self {
        let gpu = GpuLayout::new(GpuConfig::from(&config)).await.ok();
        Self {
            inner: gpu,
            cpu: Some(CpuForceLayout::new(Vec::new(), Vec::new(), config.clone())),
            nodes: Vec::new(),
            index_map: HashMap::new(),
            positions: Vec::new(),
            edges: Vec::new(),
            pinned: HashMap::new(),
            node_bounds: HashMap::new(),
            config,
            iteration: 0,
            dirty: false,
        }
    }

    pub async fn with_defaults() -> Self {
        Self::new(LayoutConfig::default()).await
    }

    pub fn add_node(&mut self, id: NoteId, pinned: bool) {
        if self.index_map.contains_key(&id) {
            return;
        }
        let idx = self.nodes.len() as u32;
        self.nodes.push(id);
        self.index_map.insert(id, idx);

        let pos = Self::random_position();
        self.positions.push(VibePos::new(pos.x, pos.y));

        if pinned {
            self.pinned.insert(id, VibePos::new(pos.x, pos.y));
        }
        self.dirty = true;
    }

    pub fn remove_node(&mut self, id: &NoteId) {
        let Some(&idx) = self.index_map.get(id) else {
            return;
        };
        self.nodes.remove(idx as usize);
        self.positions.remove(idx as usize);
        self.index_map.remove(id);
        self.pinned.remove(id);
        self.node_bounds.remove(id);

        // Rebuild index map since indices shifted
        self.index_map.clear();
        for (i, nid) in self.nodes.iter().enumerate() {
            self.index_map.insert(*nid, i as u32);
        }

        // Remove edges touching this node
        self.edges.retain(|e| {
            let src = self.nodes.get(e.source as usize);
            let tgt = self.nodes.get(e.target as usize);
            src.is_some() && tgt.is_some()
        });

        self.dirty = true;
    }

    pub fn update_links(&mut self, links: Vec<Link>) {
        self.edges = links
            .iter()
            .filter_map(|link| {
                let src = self.index_map.get(&link.source).copied()?;
                let tgt = self.index_map.get(&link.target).copied()?;
                Some(VibeEdge::new(src, tgt))
            })
            .collect();
        self.dirty = true;
    }

    pub fn set_position(&mut self, id: &NoteId, pos: Position) {
        let Some(&idx) = self.index_map.get(id) else {
            return;
        };
        self.positions[idx as usize] = VibePos::new(pos.x as f32, pos.y as f32);
        // Also update pinned position if this node is pinned
        if self.pinned.contains_key(id) {
            self.pinned
                .insert(*id, VibePos::new(pos.x as f32, pos.y as f32));
        }
        self.dirty = true;
    }

    /// Set the bounding box for a single node (used for collision avoidance).
    pub fn set_node_bounds(&mut self, id: NoteId, bounds: Bounds) {
        self.node_bounds.insert(id, bounds);
    }

    /// Batch-set bounding boxes for all nodes.
    pub fn set_bounds(&mut self, bounds: HashMap<NoteId, Bounds>) {
        self.node_bounds = bounds;
    }

    /// Get the bounding box for a node, if one was set.
    pub fn get_bounds(&self, id: &NoteId) -> Option<Bounds> {
        self.node_bounds.get(id).copied()
    }

    pub fn pin(&mut self, id: &NoteId, pinned: bool) {
        if pinned {
            if let Some(&idx) = self.index_map.get(id) {
                self.pinned.insert(*id, self.positions[idx as usize]);
            }
        } else {
            self.pinned.remove(id);
        }
    }

    pub fn get_position(&self, id: &NoteId) -> Option<Position> {
        let &idx = self.index_map.get(id)?;
        let p = self.positions[idx as usize];
        Some(Position::new(p.x as f64, p.y as f64))
    }

    pub fn all_positions(&self) -> HashMap<NoteId, Position> {
        self.nodes
            .iter()
            .map(|id| {
                let pos = self.get_position(id).unwrap();
                (*id, pos)
            })
            .collect()
    }

    pub fn is_converged(&self) -> bool {
        self.iteration >= self.config.max_iterations
    }

    pub fn iteration_count(&self) -> usize {
        self.iteration
    }

    /// Run a single iteration of the force-directed layout.
    ///
    /// Returns the average displacement of non-pinned nodes.
    pub fn step(&mut self) -> f64 {
        if self.nodes.is_empty() {
            self.iteration += 1;
            return 0.0;
        }

        let old_positions: Vec<VibePos> = self.positions.clone();

        let mut gpu_stepped = false;

        // GPU step (skipped when no backend or empty edges as wgpu rejects
        // zero-sized storage buffers).
        if let Some(ref mut gpu) = self.inner {
            if !self.edges.is_empty() {
                // Re-init when the graph structure changed
                if self.dirty {
                    let _ = gpu.init(self.positions.clone(), self.edges.clone());
                    gpu.start();
                    self.dirty = false;
                }

                match gpu.step() {
                    Ok(new_positions) => {
                        self.positions.copy_from_slice(new_positions);
                        gpu_stepped = true;
                    }
                    Err(e) => {
                        tracing::error!("GPU layout step failed: {e}");
                    }
                }
            }
        }

        // CPU fallback when GPU is unavailable or failed.
        if !gpu_stepped {
            if let Some(ref mut cpu) = self.cpu {
                if self.dirty {
                    let cpu_edges: Vec<(usize, usize)> = self
                        .edges
                        .iter()
                        .map(|e| (e.source as usize, e.target as usize))
                        .collect();
                    let cpu_positions: Vec<Position> = self
                        .positions
                        .iter()
                        .map(|p| Position::new(p.x as f64, p.y as f64))
                        .collect();
                    *cpu = CpuForceLayout::new(cpu_positions, cpu_edges, self.config.clone());
                    self.dirty = false;
                }

                let new_positions = cpu.step();
                for (i, pos) in new_positions.iter().enumerate() {
                    self.positions[i] = VibePos::new(pos.x as f32, pos.y as f32);
                }
            }
        }

        // Restore pinned node positions (undoes any GPU movement).
        for (id, pin_pos) in &self.pinned {
            if let Some(&idx) = self.index_map.get(id) {
                self.positions[idx as usize] = *pin_pos;
            }
        }

        // Resolve collisions among nodes with explicit bounds.
        // Runs even without edges (nodes can overlap at initial positions).
        if !self.node_bounds.is_empty() {
            self.resolve_collisions();
        }

        // Calculate average displacement for non-pinned nodes.
        let mut total_disp = 0.0f64;
        let mut count = 0u32;
        for (i, nid) in self.nodes.iter().enumerate() {
            if self.pinned.contains_key(nid) {
                continue;
            }
            let old = &old_positions[i];
            let new = &self.positions[i];
            let dx = new.x as f64 - old.x as f64;
            let dy = new.y as f64 - old.y as f64;
            total_disp += (dx * dx + dy * dy).sqrt();
            count += 1;
        }

        self.iteration += 1;
        if count == 0 {
            0.0
        } else {
            total_disp / count as f64
        }
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

    /// Incremental step that moves only the neighborhood of a given node.
    ///
    /// The neighborhood is the set of nodes within `neighborhood_radius` of
    /// `id`, plus any spring partners of those nodes (which act as anchors).
    pub fn step_neighborhood(&mut self, id: &NoteId) -> f64 {
        let Some(&center_u32) = self.index_map.get(id) else {
            return 0.0;
        };
        let center = center_u32 as usize;
        if self.nodes.is_empty() {
            self.iteration += 1;
            return 0.0;
        }

        let radius = self.config.neighborhood_radius;

        let old_positions: Vec<VibePos> = self.positions.clone();

        // Re-sync the CPU layout from engine positions so forces stay
        // continuous with prior steps, then run the localized pass.
        let moved = {
            let cpu = self.cpu.get_or_insert_with(|| {
                CpuForceLayout::new(
                    self.positions
                        .iter()
                        .map(|p| Position::new(p.x as f64, p.y as f64))
                        .collect(),
                    self.edges
                        .iter()
                        .map(|e| (e.source as usize, e.target as usize))
                        .collect(),
                    self.config.clone(),
                )
            });
            let start = cpu.positions().len();
            if start != self.positions.len() {
                // Graph structure changed since the last rebuild; reconstruct.
                *cpu = CpuForceLayout::new(
                    self.positions
                        .iter()
                        .map(|p| Position::new(p.x as f64, p.y as f64))
                        .collect(),
                    self.edges
                        .iter()
                        .map(|e| (e.source as usize, e.target as usize))
                        .collect(),
                    self.config.clone(),
                );
            }
            cpu.step_neighborhood(center, radius)
        };

        // Copy the moved nodes back into the engine position store.
        for &i in &moved {
            let p = self
                .cpu
                .as_ref()
                .expect("cpu layout exists in this branch")
                .positions()[i];
            self.positions[i] = VibePos::new(p.x as f32, p.y as f32);
        }

        // Restore pinned nodes within the neighborhood.
        for (pid, pin_pos) in &self.pinned {
            if let Some(&idx) = self.index_map.get(pid) {
                if moved.contains(&(idx as usize)) {
                    self.positions[idx as usize] = *pin_pos;
                }
            }
        }

        // Neighborhood-scoped collision resolution (spatial hash, local cell).
        self.resolve_collisions_neighborhood(center, radius);

        // Average displacement of the moved, non-pinned nodes.
        let mut total_disp = 0.0f64;
        let mut count = 0u32;
        for &i in &moved {
            let nid = self.nodes[i];
            if self.pinned.contains_key(&nid) {
                continue;
            }
            let old = &old_positions[i];
            let new = &self.positions[i];
            let dx = new.x as f64 - old.x as f64;
            let dy = new.y as f64 - old.y as f64;
            total_disp += (dx * dx + dy * dy).sqrt();
            count += 1;
        }

        self.iteration += 1;
        if count == 0 {
            0.0
        } else {
            total_disp / count as f64
        }
    }

    /// Push apart overlapping node bounding boxes.
    ///
    /// Runs several iterative passes. In each pass, every pair of overlapping
    /// nodes is pushed apart along the center-to-center vector, clamped to
    /// `max_collision_push` per pass. The margin controls how much extra
    /// separation to maintain beyond bare overlap.
    fn resolve_collisions(&mut self) {
        let margin = self.config.collision_margin;
        let max_push = self.config.max_collision_push;
        let passes = self.config.collision_passes;

        // Work in f64 to avoid precision issues with many small pushes.
        let mut pos_f64: Vec<Position> = self
            .positions
            .iter()
            .map(|p| Position::new(p.x as f64, p.y as f64))
            .collect();

        // The search radius must span the widest possible overlapping pair:
        // two nodes at their max half-diagonal separation, plus the margin.
        let mut max_half_diag = 0.0f64;
        for bounds in self.node_bounds.values() {
            let half_diag =
                (bounds.width * bounds.width + bounds.height * bounds.height).sqrt() * 0.5;
            max_half_diag = max_half_diag.max(half_diag);
        }
        let search_radius = max_half_diag * 2.0 + margin;

        for _pass in 0..passes {
            let mut any_resolved = false;
            // Rebuild the index each pass so recently pushed nodes are found
            // in their new cells on the next iteration.
            let hash = SpatialHash::new(search_radius, pos_f64.clone());

            for i in 0..self.nodes.len() {
                if self.pinned.contains_key(&self.nodes[i]) {
                    continue;
                }
                let Some(bounds_i) = self.node_bounds.get(&self.nodes[i]) else {
                    continue;
                };
                let a = pos_f64[i];
                let center_a =
                    Position::new(a.x + bounds_i.width * 0.5, a.y + bounds_i.height * 0.5);

                for j in hash.within(center_a, search_radius) {
                    if j <= i {
                        continue;
                    }
                    if self.pinned.contains_key(&self.nodes[j]) {
                        continue;
                    }
                    let Some(bounds_j) = self.node_bounds.get(&self.nodes[j]) else {
                        continue;
                    };

                    let b = pos_f64[j];

                    if !bounds_i.overlaps(&a, bounds_j, &b) {
                        continue;
                    }

                    // Compute separation direction (center to center).
                    let ca_x = a.x + bounds_i.width * 0.5;
                    let ca_y = a.y + bounds_i.height * 0.5;
                    let cb_x = b.x + bounds_j.width * 0.5;
                    let cb_y = b.y + bounds_j.height * 0.5;

                    let dx = cb_x - ca_x;
                    let dy = cb_y - ca_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    let (nx, ny) = if dist < 1e-6 {
                        // Coincident centers: fall back to a deterministic
                        // direction derived from the node indices.
                        let angle = (i * 2654435761 + j * 2246822519) as f64;
                        (angle.cos(), angle.sin())
                    } else {
                        (dx / dist, dy / dist)
                    };

                    // Target separation distance: half-diagonal sum + margin.
                    let half_diag_i = (bounds_i.width * bounds_i.width
                        + bounds_i.height * bounds_i.height)
                        .sqrt()
                        * 0.5;
                    let half_diag_j = (bounds_j.width * bounds_j.width
                        + bounds_j.height * bounds_j.height)
                        .sqrt()
                        * 0.5;
                    let target = half_diag_i + half_diag_j + margin;

                    if dist >= target {
                        continue;
                    }

                    let push = ((target - dist) * 0.5).min(max_push);

                    pos_f64[i].x -= nx * push;
                    pos_f64[i].y -= ny * push;
                    pos_f64[j].x += nx * push;
                    pos_f64[j].y += ny * push;

                    any_resolved = true;
                }
            }

            if !any_resolved {
                break;
            }
        }

        // Write back to the GPU positions.
        for (i, p) in pos_f64.iter().enumerate() {
            self.positions[i] = VibePos::new(p.x as f32, p.y as f32);
        }
    }

    /// Push apart overlapping bounding boxes among nodes near a center.
    fn resolve_collisions_neighborhood(&mut self, center: usize, radius: f64) {
        let margin = self.config.collision_margin;
        let max_push = self.config.max_collision_push;
        let passes = self.config.collision_passes;

        let mut max_half_diag = 0.0f64;
        for bounds in self.node_bounds.values() {
            let half_diag =
                (bounds.width * bounds.width + bounds.height * bounds.height).sqrt() * 0.5;
            max_half_diag = max_half_diag.max(half_diag);
        }
        let search_radius = max_half_diag * 2.0 + margin;

        // The spatial hash cell covers the search radius around the center so
        // the neighborhood query includes the anchor ring of overlapping nodes.
        let mut pos_f64: Vec<Position> = self
            .positions
            .iter()
            .map(|p| Position::new(p.x as f64, p.y as f64))
            .collect();
        let center_pos = pos_f64[center];
        let initial_hash = SpatialHash::new(search_radius, pos_f64.clone());
        let neighborhood: std::collections::HashSet<usize> = initial_hash
            .within(center_pos, radius + search_radius)
            .into_iter()
            .collect();

        for _pass in 0..passes {
            let mut any_resolved = false;
            let hash = SpatialHash::new(search_radius, pos_f64.clone());
            let mut neighbors: Vec<usize> = hash.within(center_pos, radius + search_radius);
            neighbors.sort_unstable();
            neighbors.dedup();

            for &i in &neighbors {
                if !neighborhood.contains(&i) {
                    continue;
                }
                if self.pinned.contains_key(&self.nodes[i]) {
                    continue;
                }
                let Some(bounds_i) = self.node_bounds.get(&self.nodes[i]) else {
                    continue;
                };
                let a = pos_f64[i];
                let center_a =
                    Position::new(a.x + bounds_i.width * 0.5, a.y + bounds_i.height * 0.5);

                for j in hash.within(center_a, search_radius) {
                    if j <= i {
                        continue;
                    }
                    if self.pinned.contains_key(&self.nodes[j]) {
                        continue;
                    }
                    let Some(bounds_j) = self.node_bounds.get(&self.nodes[j]) else {
                        continue;
                    };
                    let b = pos_f64[j];

                    if !bounds_i.overlaps(&a, bounds_j, &b) {
                        continue;
                    }

                    let ca_x = a.x + bounds_i.width * 0.5;
                    let ca_y = a.y + bounds_i.height * 0.5;
                    let cb_x = b.x + bounds_j.width * 0.5;
                    let cb_y = b.y + bounds_j.height * 0.5;

                    let dx = cb_x - ca_x;
                    let dy = cb_y - ca_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    let (nx, ny) = if dist < 1e-6 {
                        let angle = (i * 2654435761 + j * 2246822519) as f64;
                        (angle.cos(), angle.sin())
                    } else {
                        (dx / dist, dy / dist)
                    };

                    let half_diag_i = (bounds_i.width * bounds_i.width
                        + bounds_i.height * bounds_i.height)
                        .sqrt()
                        * 0.5;
                    let half_diag_j = (bounds_j.width * bounds_j.width
                        + bounds_j.height * bounds_j.height)
                        .sqrt()
                        * 0.5;
                    let target = half_diag_i + half_diag_j + margin;

                    if dist >= target {
                        continue;
                    }

                    let push = ((target - dist) * 0.5).min(max_push);

                    if neighborhood.contains(&j) {
                        pos_f64[i].x -= nx * push;
                        pos_f64[i].y -= ny * push;
                        pos_f64[j].x += nx * push;
                        pos_f64[j].y += ny * push;
                    } else {
                        // j is an anchor outside the neighborhood; only i moves.
                        pos_f64[i].x -= nx * push * 2.0;
                        pos_f64[i].y -= ny * push * 2.0;
                    }

                    any_resolved = true;
                }
            }

            if !any_resolved {
                break;
            }
        }

        for (i, p) in pos_f64.iter().enumerate() {
            if neighborhood.contains(&i) {
                self.positions[i] = VibePos::new(p.x as f32, p.y as f32);
            }
        }
    }

    fn random_position() -> VibePos {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let seed = nanos as f64;
        let angle = seed * std::f64::consts::TAU / 1_000_000_000.0;
        let radius = 50.0 + (seed % 200.0);
        VibePos::new((angle.cos() * radius) as f32, (angle.sin() * radius) as f32)
    }
}
