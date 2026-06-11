use std::sync::Arc;

use async_trait::async_trait;
use candle_core::{Device, Tensor};
use penumbra_core::embed::{Embedding, EmbeddingProvider};
use penumbra_core::error::{PenumbraError, Result};
use tokenizers::Tokenizer;

const MODEL_ID: &str = "Snowflake/snowflake-arctic-embed-xs";
const DEFAULT_SEQUENCE_LENGTH: usize = 512;

pub struct CandleEmbedder {
    model: ArcticEmbedXS,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEmbedder {
    pub fn new(model: ArcticEmbedXS, tokenizer: Tokenizer, device: Device) -> Self {
        Self {
            model,
            tokenizer,
            device,
        }
    }

    /// Load the model and tokenizer from HuggingFace Hub or local cache.
    pub async fn load() -> Result<Self> {
        let api = hf_hub::api::sync::Api::new()?;
        let model_path = api.model(MODEL_ID.to_string()).get("model.safetensors")?;
        let config_path = api.model(MODEL_ID.to_string()).get("config.json")?;
        let tokenizer_path = api.model(MODEL_ID.to_string()).get("tokenizer.json")?;

        let device = Device::Cpu;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| PenumbraError::Embedding(format!("failed to load tokenizer: {e}")))?;

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let hidden_size = config["hidden_size"].as_u64().unwrap_or(384) as usize;

        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &[model_path],
                candle_core::DType::F32,
                &device,
            )?
        };

        let model = ArcticEmbedXS::new(hidden_size, vb)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
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

        let input = Tensor::new(ids, &self.device)?.unsqueeze(0)?;
        let mask = Tensor::ones(&[1, len], candle_core::DType::U32, &self.device)?;

        let embedding = self.model.forward(&input, &mask)?;

        // L2-normalize
        let norm = embedding.sqr()?.sum_all()?.sqrt()?.to_scalar::<f32>()?;
        let normalized = embedding.broadcast_div(&Tensor::new(norm, &self.device)?)?;

        let vec: Vec<f32> = normalized.flatten_all()?.to_vec1()?;
        Ok(vec)
    }

    fn dimensions(&self) -> usize {
        self.model.hidden_size
    }
}

/// Minimal BERT-style embedding model.
///
/// This is a simplified forward pass that will be replaced with the actual
/// ONNX-transformed or Candle-native model once the architecture is confirmed.
pub struct ArcticEmbedXS {
    hidden_size: usize,
    embed: candle_nn::Embedding,
    encoder: candle_nn::Linear,
}

impl ArcticEmbedXS {
    fn new(hidden_size: usize, vb: candle_nn::VarBuilder) -> Result<Self> {
        let embed = candle_nn::embedding(30522, hidden_size, &vb.pp("embeddings.word_embeddings"))?;
        let encoder = candle_nn::linear(hidden_size, hidden_size, &vb.pp("encoder.layer.0"))?;

        Ok(Self {
            hidden_size,
            embed,
            encoder,
        })
    }

    fn forward(&self, input_ids: &Tensor, _attention_mask: &Tensor) -> Result<Tensor> {
        let x = self.embed.forward(input_ids)?;
        let x = x.mean(1)?; // Mean pooling
        let x = self.encoder.forward(&x)?;
        Ok(x)
    }
}
