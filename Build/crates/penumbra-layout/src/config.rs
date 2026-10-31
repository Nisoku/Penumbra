use vibe_graph_layout_gpu::LayoutConfig as GpuConfig;

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
    /// Default bounding box (world pixels) assigned to a node when it is added
    /// without an explicit box.
    pub default_node_bounds: (f64, f64),
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
            default_node_bounds: (180.0, 120.0),
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
