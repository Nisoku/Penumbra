use async_trait::async_trait;
use candle_core::{Device, Error as CandleError, Tensor};
use candle_nn::Module;
use penumbra_core::embed::{Embedding, EmbeddingProvider};
use penumbra_core::error::{PenumbraError, Result};
use tokenizers::Tokenizer;

#[cfg(feature = "candle-load")]
const MODEL_ID: &str = "Snowflake/snowflake-arctic-embed-xs";
const DEFAULT_SEQUENCE_LENGTH: usize = 512;

fn e_msg(e: impl std::fmt::Display) -> PenumbraError {
    PenumbraError::Embedding(e.to_string())
}

pub struct CandleEmbedder {
    model: ArcticEmbedXS,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEmbedder {
    /// Build an embedder from pre-constructed components.
    pub fn new(model: ArcticEmbedXS, tokenizer: Tokenizer, device: Device) -> Self {
        Self {
            model,
            tokenizer,
            device,
        }
    }

    /// Build an embedder from raw model bytes.
    pub fn from_bytes(
        model_bytes: &[u8],
        config_bytes: &[u8],
        tokenizer_bytes: &[u8],
    ) -> Result<Self> {
        let device = Device::Cpu;

        let tokenizer = Tokenizer::from_bytes(tokenizer_bytes)
            .map_err(|e| PenumbraError::Embedding(format!("failed to load tokenizer: {e}")))?;

        let config: serde_json::Value = serde_json::from_slice(config_bytes).map_err(e_msg)?;
        let hidden_size = config["hidden_size"].as_u64().unwrap_or(384) as usize;
        let vocab_size = config["vocab_size"].as_u64().unwrap_or(30522) as usize;

        let vb = candle_nn::VarBuilder::from_buffered_safetensors(
            model_bytes.to_vec(),
            candle_core::DType::F32,
            &device,
        )
        .map_err(e_msg)?;

        let model = ArcticEmbedXS::new(vocab_size, hidden_size, vb).map_err(e_msg)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Download from HuggingFace and build. Native only.
    #[cfg(feature = "candle-load")]
    pub async fn load() -> Result<Self> {
        let client = reqwest::Client::new();

        let model_bytes = Self::download(&client, "model.safetensors").await?;
        let config_bytes = Self::download(&client, "config.json").await?;
        let tokenizer_bytes = Self::download(&client, "tokenizer.json").await?;

        Self::from_bytes(&model_bytes, &config_bytes, &tokenizer_bytes)
    }

    #[cfg(feature = "candle-load")]
    async fn download(client: &reqwest::Client, filename: &str) -> Result<Vec<u8>> {
        let url = format!("https://huggingface.co/{MODEL_ID}/resolve/main/{filename}");
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PenumbraError::Embedding(format!("download failed: {e}")))?;
        if !response.status().is_success() {
            return Err(PenumbraError::Embedding(format!(
                "HTTP {} downloading {filename}",
                response.status()
            )));
        }
        response
            .bytes()
            .await
            .map_err(|e| PenumbraError::Embedding(format!("read response failed: {e}")))
            .map(|b| b.to_vec())
    }
}

#[async_trait]
impl EmbeddingProvider for CandleEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let tokens = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| PenumbraError::Embedding(format!("tokenization failed: {e}")))?;

        let ids = tokens.get_ids();
        let len = ids.len().min(DEFAULT_SEQUENCE_LENGTH);
        let ids = &ids[..len];

        let input = Tensor::new(ids, &self.device)
            .map_err(e_msg)?
            .unsqueeze(0)
            .map_err(e_msg)?;
        let mask = Tensor::ones(&[1, len], candle_core::DType::U32, &self.device).map_err(e_msg)?;

        let embedding = self.model.forward(&input, &mask).map_err(e_msg)?;

        let squared_sum = embedding
            .powf(2.0)
            .map_err(e_msg)?
            .sum_all()
            .map_err(e_msg)?;
        let norm = squared_sum.powf(0.5).map_err(e_msg)?;
        let normalized = embedding.broadcast_div(&norm).map_err(e_msg)?;

        let vec: Vec<f32> = normalized
            .flatten_all()
            .map_err(e_msg)?
            .to_vec1()
            .map_err(e_msg)?;
        Ok(vec)
    }

    fn dimensions(&self) -> usize {
        self.model.hidden_size
    }
}

/// Minimal BERT-style embedding model.
pub struct ArcticEmbedXS {
    hidden_size: usize,
    embed: candle_nn::Embedding,
    encoder: candle_nn::Linear,
}

impl ArcticEmbedXS {
    pub fn new(
        vocab_size: usize,
        hidden_size: usize,
        vb: candle_nn::VarBuilder,
    ) -> std::result::Result<Self, CandleError> {
        let embed =
            candle_nn::embedding(vocab_size, hidden_size, vb.pp("embeddings.word_embeddings"))?;
        let encoder = candle_nn::linear(hidden_size, hidden_size, vb.pp("encoder.layer.0"))?;

        Ok(Self {
            hidden_size,
            embed,
            encoder,
        })
    }

    pub fn forward(
        &self,
        input_ids: &Tensor,
        _attention_mask: &Tensor,
    ) -> std::result::Result<Tensor, CandleError> {
        let x = self.embed.forward(input_ids)?;
        let x = x.mean(1)?;
        let x = self.encoder.forward(&x)?;
        Ok(x)
    }
}
