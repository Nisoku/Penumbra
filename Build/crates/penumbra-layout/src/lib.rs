mod collision;
mod config;
mod cpu;

use std::collections::{HashMap, HashSet};

use penumbra_core::link::Link;
use penumbra_core::note::NoteId;
use penumbra_core::position::{Bounds, Position};
use vibe_graph_layout_gpu::{
    Edge as VibeEdge, GpuLayout, LayoutConfig as GpuConfig, Position as VibePos,
};

pub use config::LayoutConfig;
use cpu::CpuForceLayout;

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

        let pos = Self::seed_position(idx as usize);
        self.positions.push(VibePos::new(pos.x, pos.y));

        // Assign the default card-sized box so collision avoidance keeps this
        // node from overlapping others until the UI supplies a measured box.
        let (w, h) = self.config.default_node_bounds;
        self.node_bounds.insert(id, Bounds::new(w, h));

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

    /// Drop a node's bounding box so it is excluded from collision avoidance.
    pub fn clear_node_bounds(&mut self, id: &NoteId) {
        self.node_bounds.remove(id);
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

            if let Some(ref mut cpu) = self.cpu {
                let synced: Vec<Position> = self
                    .positions
                    .iter()
                    .map(|p| Position::new(p.x as f64, p.y as f64))
                    .collect();
                cpu.set_positions(&synced);
            }
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

    /// The node ids currently pinned, for collision passes that must leave
    /// pinned nodes fixed.
    fn pinned_ids(&self) -> HashSet<NoteId> {
        self.pinned.keys().copied().collect()
    }

    /// Positions converted to the f64 domain the collision solver works in.
    fn positions_f64(&self) -> Vec<Position> {
        self.positions
            .iter()
            .map(|p| Position::new(p.x as f64, p.y as f64))
            .collect()
    }

    /// Push apart overlapping node bounding boxes across the whole graph. See
    /// [`collision::resolve_all`] for the per-pass mechanics.
    fn resolve_collisions(&mut self) {
        let pinned = self.pinned_ids();
        let field = collision::CollisionField {
            nodes: &self.nodes,
            node_bounds: &self.node_bounds,
            pinned: &pinned,
            config: &self.config,
        };
        let resolved = collision::resolve_all(&field, &self.positions_f64());
        for (i, p) in resolved.iter().enumerate() {
            self.positions[i] = VibePos::new(p.x as f32, p.y as f32);
        }
    }

    /// Push apart overlapping bounding boxes among nodes near a center. See
    /// [`collision::resolve_neighborhood`] for the anchor-ring behavior.
    fn resolve_collisions_neighborhood(&mut self, center: usize, radius: f64) {
        let pinned = self.pinned_ids();
        let field = collision::CollisionField {
            nodes: &self.nodes,
            node_bounds: &self.node_bounds,
            pinned: &pinned,
            config: &self.config,
        };
        let resolved = collision::resolve_neighborhood(&field, &self.positions_f64(), center, radius);
        for (i, p) in resolved.iter().enumerate() {
            self.positions[i] = VibePos::new(p.x as f32, p.y as f32);
        }
    }

    fn seed_position(index: usize) -> VibePos {
        // 137.5 degrees in radians: the golden angle HEHEHEE
        const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
        const SPREAD: f64 = 140.0;
        let n = index as f64;
        let angle = n * GOLDEN_ANGLE;
        let radius = SPREAD * (n + 0.5).sqrt();
        VibePos::new((angle.cos() * radius) as f32, (angle.sin() * radius) as f32)
    }
}
