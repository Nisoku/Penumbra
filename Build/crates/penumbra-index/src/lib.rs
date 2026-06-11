use penumbra_core::error::Result;
use penumbra_core::note::NoteId;

/// A search result from a vector index.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: NoteId,
    pub score: f32,
}

/// Trait for approximate nearest-neighbor vector search.
pub trait VectorIndex: Send + Sync {
    fn insert(&mut self, id: NoteId, vector: &[f32]) -> Result<()>;
    fn remove(&mut self, id: &NoteId) -> Result<()>;
    fn search(&self, vector: &[f32], k: usize) -> Result<Vec<SearchHit>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub mod usearch_backend;
pub use usearch_backend::USearchIndex;
