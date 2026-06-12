use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RenderNode {
    pub id: NoteId,
    pub position: Position,
    pub title: String,
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RenderEdge {
    pub source: NoteId,
    pub target: NoteId,
    pub opacity: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RenderState {
    pub camera: Camera,
    pub nodes: Vec<RenderNode>,
    pub edges: Vec<RenderEdge>,
    pub hovered_node: Option<NoteId>,
    pub selected_node: Option<NoteId>,
}
