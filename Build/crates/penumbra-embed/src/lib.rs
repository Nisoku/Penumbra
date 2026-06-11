mod simple;

pub use simple::SimpleEmbedder;

#[cfg(feature = "candle")]
pub mod candle;

use async_trait::async_trait;
use penumbra_core::embed::{Embedding, EmbeddingProvider};
use penumbra_core::error::Result;

/// An embedder that returns zero-filled vectors of the configured dimension.
///
/// Useful as a stub during development or when no ML pipeline is configured.
pub struct NullEmbedder {
    dims: usize,
}

impl NullEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }
}

#[async_trait]
impl EmbeddingProvider for NullEmbedder {
    async fn embed_text(&self, _text: &str) -> Result<Embedding> {
        Ok(vec![0.0f32; self.dims])
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}
