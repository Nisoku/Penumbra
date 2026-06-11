use std::collections::HashMap;

use penumbra_core::error::{PenumbraError, Result};
use penumbra_core::note::NoteId;

use crate::{SearchHit, VectorIndex};

/// A USearch-backed vector index using HNSW for approximate nearest-neighbor
/// search. Fast on both native and WASM targets.
pub struct USearchIndex {
    inner: usearch::Index,
    /// Mapping from external NoteId to USearch's u64 keys.
    keys: HashMap<NoteId, u64>,
    /// Reverse mapping for removal.
    rev: HashMap<u64, NoteId>,
    next_key: u64,
    dims: usize,
}

impl USearchIndex {
    pub fn new(dims: usize) -> Result<Self> {
        let inner = usearch::Index::new(&usearch::IndexOptions {
            dimensions: dims,
            metric: usearch::MetricKind::Cos,
            quantization: usearch::ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        })
        .map_err(|e| PenumbraError::Index(format!("usearch init failed: {e}")))?;

        inner
            .reserve(1024)
            .map_err(|e| PenumbraError::Index(format!("usearch reserve failed: {e}")))?;

        Ok(Self {
            inner,
            keys: HashMap::new(),
            rev: HashMap::new(),
            next_key: 1,
            dims,
        })
    }

    fn alloc_key(&mut self) -> u64 {
        let key = self.next_key;
        self.next_key += 1;
        key
    }
}

impl VectorIndex for USearchIndex {
    fn insert(&mut self, id: NoteId, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dims {
            return Err(PenumbraError::Index(format!(
                "expected {}-dimensional vector, got {}",
                self.dims,
                vector.len()
            )));
        }

        // Remove existing entry for this NoteId if any
        if let Some(&old_key) = self.keys.get(&id) {
            let _ = self.inner.remove(old_key);
            self.rev.remove(&old_key);
        }

        let key = self.alloc_key();
        self.inner
            .add(key, vector)
            .map_err(|e| PenumbraError::Index(format!("usearch add failed: {e}")))?;

        self.keys.insert(id, key);
        self.rev.insert(key, id);
        Ok(())
    }

    fn remove(&mut self, id: &NoteId) -> Result<()> {
        if let Some(&key) = self.keys.get(id) {
            self.inner
                .remove(key)
                .map_err(|e| PenumbraError::Index(format!("usearch remove failed: {e}")))?;
            self.keys.remove(id);
            self.rev.remove(&key);
        }
        Ok(())
    }

    fn search(&self, vector: &[f32], k: usize) -> Result<Vec<SearchHit>> {
        let results = self
            .inner
            .search(vector, k)
            .map_err(|e| PenumbraError::Index(format!("usearch search failed: {e}")))?;

        let hits: Vec<SearchHit> = results
            .keys
            .into_iter()
            .zip(results.distances)
            .filter_map(|(key, distance)| {
                // USearch cosine distance: 1 - cos_sim, so convert back
                let score = 1.0 - distance;
                self.rev.get(&key).map(|&id| SearchHit { id, score })
            })
            .collect();

        Ok(hits)
    }

    fn len(&self) -> usize {
        self.keys.len()
    }
}
