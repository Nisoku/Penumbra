use crate::{SearchHit, VectorIndex};
use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::note::NoteId;
use ruvector_core::types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery, VectorEntry};
use ruvector_core::VectorDB;

/// A vector index backed by `ruvector-core` (pure Rust, WASM-compatible).
pub struct RuvectorIndex {
    inner: VectorDB,
    dims: usize,
}

impl RuvectorIndex {
    /// Create a new index with the given vector dimensionality.
    ///
    /// Uses Cosine distance and HNSW (falls back to flat index on WASM).
    pub fn new(dims: usize) -> Result<Self> {
        let options = DbOptions {
            dimensions: dims,
            distance_metric: DistanceMetric::Cosine,
            hnsw_config: Some(HnswConfig {
                m: 16,
                ef_construction: 128,
                ef_search: 64,
                max_elements: 100_000,
            }),
            ..Default::default()
        };
        let inner = VectorDB::new(options)
            .map_err(|e| PenumbraError::Index(format!("ruvector init: {e}")))?;
        Ok(Self { inner, dims })
    }
}

impl VectorIndex for RuvectorIndex {
    fn insert(&mut self, id: NoteId, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dims {
            return Err(PenumbraError::Index(format!(
                "expected {}-dim vector, got {}",
                self.dims,
                vector.len()
            )));
        }
        // Remove existing entry first to avoid index duplicates
        let _ = self.inner.delete(&id.to_string());
        let entry = VectorEntry {
            id: Some(id.to_string()),
            vector: vector.to_vec(),
            metadata: None,
        };
        self.inner
            .insert(entry)
            .map_err(|e| PenumbraError::Index(format!("ruvector insert: {e}")))?;
        Ok(())
    }

    fn remove(&mut self, id: &NoteId) -> Result<()> {
        self.inner
            .delete(&id.to_string())
            .map_err(|e| PenumbraError::Index(format!("ruvector delete: {e}")))?;
        Ok(())
    }

    fn search(&self, vector: &[f32], k: usize) -> Result<Vec<SearchHit>> {
        if vector.len() != self.dims {
            return Err(PenumbraError::Index(format!(
                "expected {}-dim vector, got {}",
                self.dims,
                vector.len()
            )));
        }
        let query = SearchQuery {
            vector: vector.to_vec(),
            k,
            filter: None,
            ef_search: None,
        };
        let results = self
            .inner
            .search(query)
            .map_err(|e| PenumbraError::Index(format!("ruvector search: {e}")))?;
        let hits: Vec<SearchHit> = results
            .into_iter()
            .filter_map(|r| {
                // ruvector-core Cosine distance = 1 - cos_sim; convert to similarity score
                let id = NoteId::from_raw(uuid::Uuid::parse_str(&r.id).ok()?);
                Some(SearchHit {
                    id,
                    score: 1.0 - r.score,
                })
            })
            .collect();
        Ok(hits)
    }

    fn len(&self) -> usize {
        self.inner.len().unwrap_or(0)
    }
}
