use async_trait::async_trait;
use penumbra_core::embed::{Embedding, EmbeddingProvider};
use penumbra_core::error::Result;

/// N-gram order used by [`SimpleEmbedder`].
const NGRAM: usize = 3;

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Deterministic 64-bit FNV-1a hash over `bytes`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// A lightweight embedder that produces deterministic embeddings from text
/// using character n-gram feature hashing.
///
/// This is not semantically meaningful. It exists for development, testing,
/// and environments where the Candle ML pipeline is unavailable.
pub struct SimpleEmbedder {
    dims: usize,
}

impl SimpleEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims: dims.max(1) }
    }

    pub fn new_384() -> Self {
        Self { dims: 384 }
    }
}

#[async_trait]
impl EmbeddingProvider for SimpleEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let mut vec = vec![0.0f32; self.dims];
        let lower: Vec<char> = text.to_lowercase().chars().collect();

        if lower.len() >= NGRAM {
            for ngram in lower.windows(NGRAM) {
                let mut buf = [0u8; 12];
                let mut len = 0;
                for c in ngram {
                    let mut tmp = [0u8; 4];
                    for &byte in c.encode_utf8(&mut tmp).as_bytes() {
                        buf[len] = byte;
                        len += 1;
                    }
                }
                let hash = fnv1a(&buf[..len]);
                vec[(hash as usize) % self.dims] += 1.0;
            }
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
