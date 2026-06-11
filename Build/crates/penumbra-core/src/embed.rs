use async_trait::async_trait;

use crate::error::Result;
use crate::note::Note;

/// A vector of floating-point values representing an embedding.
pub type Embedding = Vec<f32>;

/// Provider of text embeddings for semantic search and auto-linking.
///
/// The implementation can be Candle-based (with a local ML model),
/// a mock for testing, or any other backend. The trait is async so
/// that the caller never blocks regardless of the backend.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Compute an embedding for a single piece of text.
    async fn embed_text(&self, text: &str) -> Result<Embedding>;

    /// Compute embeddings for multiple texts in a batch.
    ///
    /// The default implementation calls `embed_text` for each item,
    /// but providers may override with a batched inference for efficiency.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text).await?);
        }
        Ok(results)
    }

    /// Convenience: embed the combined title + body of a note.
    async fn embed_note(&self, note: &Note) -> Result<Embedding> {
        let text = format!("{} {}", note.title, note.body);
        self.embed_text(&text).await
    }

    /// The dimensionality (this was fun researching) of the embeddings this provider produces.
    fn dimensions(&self) -> usize;
}
