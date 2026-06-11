use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use async_trait::async_trait;
use penumbra_core::embed::{Embedding, EmbeddingProvider};
use penumbra_core::error::Result;

/// A lightweight embedder that produces deterministic embeddings from text
/// using character n-gram hashing.
///
/// This is not semantically meaningful. It exists for development, testing,
/// and environments where the Candle ML pipeline is unavailable. The output
/// is a fixed-dimension vector where each position is a hash of an n-gram
/// from the input.
pub struct SimpleEmbedder {
    dims: usize,
}

impl SimpleEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }

    pub fn new_384() -> Self {
        Self { dims: 384 }
    }
}

#[async_trait]
impl EmbeddingProvider for SimpleEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let mut vec = vec![0.0f32; self.dims];
        let lower = text.to_lowercase();

        // Slide character trigrams and hash each into a bucket
        for trigram in lower.as_bytes().windows(3) {
            let mut hasher = DefaultHasher::new();
            trigram.hash(&mut hasher);
            let hash = hasher.finish();
            let idx = (hash as usize) % self.dims;
            vec[idx] += 1.0;
        }

        // L2-normalize so all embeddings have comparable magnitudes
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        Ok(vec)
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}
