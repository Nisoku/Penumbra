use std::collections::HashMap;

use penumbra_core::link::Link;
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use vibe_graph_layout_gpu::{
    Edge as VibeEdge, GpuLayout, LayoutConfig as GpuConfig, Position as VibePos,
};

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
/// f32 positions, Edges). GPU initialization is deferred until the first `step()`
/// call so construction is synchronous.
pub struct LayoutEngine {
    inner: Option<GpuLayout>,
    nodes: Vec<NoteId>,
    index_map: HashMap<NoteId, u32>,
    positions: Vec<VibePos>,
    edges: Vec<VibeEdge>,
    pinned: HashMap<NoteId, VibePos>,
    config: LayoutConfig,
    iteration: usize,
    dirty: bool,
    initialized: bool,
}

impl LayoutEngine {
    pub fn new(config: LayoutConfig) -> Self {
        Self {
            inner: None,
            nodes: Vec::new(),
            index_map: HashMap::new(),
            positions: Vec::new(),
            edges: Vec::new(),
            pinned: HashMap::new(),
            config,
            iteration: 0,
            dirty: false,
            initialized: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(LayoutConfig::default())
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
    /// GPU initialization happens lazily on the first call.
    pub fn step(&mut self) -> f64 {
        if self.nodes.is_empty() {
            self.iteration += 1;
            return 0.0;
        }

        // No edges means no forces so skip GPU (zero-sized storage buffers
        // are rejected by wgpu validation) and treat as a no-op step.
        if self.edges.is_empty() {
            self.iteration += 1;
            return 0.0;
        }

        // Lazy GPU initialization on first step
        if !self.initialized {
            let gpu_config = GpuConfig::from(&self.config);
            let mut gpu = match pollster::block_on(GpuLayout::new(gpu_config)) {
                Ok(g) => g,
                Err(e) => {
                    tracing::error!("Failed to initialize GPU layout: {e}");
                    self.iteration += 1;
                    return 0.0;
                }
            };
            if let Err(e) = gpu.init(self.positions.clone(), self.edges.clone()) {
                tracing::error!("Failed to init GPU layout: {e}");
                self.iteration += 1;
                return 0.0;
            }
            gpu.start();
            self.inner = Some(gpu);
            self.initialized = true;
            self.dirty = false;
        }

        // Re-init if the graph changed
        if self.dirty {
            if let Some(ref mut gpu) = self.inner {
                if gpu
                    .init(self.positions.clone(), self.edges.clone())
                    .is_err()
                {
                    self.iteration += 1;
                    return 0.0;
                }
                gpu.start();
            }
            self.dirty = false;
        }

        let old_positions: Vec<VibePos> = self.positions.clone();

        let result = self.inner.as_mut().map(|gpu| gpu.step());
        match result {
            Some(Ok(new_positions)) => {
                self.positions.copy_from_slice(new_positions);
            }
            _ => {
                self.iteration += 1;
                return 0.0;
            }
        }

        // Restore pinned node positions
        for (id, pin_pos) in &self.pinned {
            if let Some(&idx) = self.index_map.get(id) {
                self.positions[idx as usize] = *pin_pos;
            }
        }

        // Calculate average displacement for non-pinned nodes
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
    /// Note: with GPU acceleration the full graph is always computed, so this
    /// is functionally equivalent to `step()`.
    pub fn step_neighborhood(&mut self, _id: &NoteId) -> f64 {
        self.step()
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
