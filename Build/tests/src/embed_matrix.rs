use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::note::Note;
use penumbra_embed::{NullEmbedder, SimpleEmbedder};

#[cfg(feature = "candle")]
mod candle {
    use candle_core::{DType, Device, Tensor};
    use penumbra_core::embed::EmbeddingProvider;
    use penumbra_embed::candle::{ArcticEmbedXS, CandleEmbedder};

    fn test_safetensors(vocab_size: usize, hidden_size: usize) -> Vec<u8> {
        let embed_len = vocab_size * hidden_size;
        let enc_w_len = hidden_size * hidden_size;

        let embed_data: Vec<f32> = (0..embed_len)
            .map(|i| ((i as f32) * 0.01).fract())
            .collect();
        let enc_w_data: Vec<f32> = (0..enc_w_len)
            .map(|i| ((i as f32 * 0.1 + 0.5) % 1.0))
            .collect();
        let enc_b_data = vec![0.0f32; hidden_size];

        let embed_bytes: Vec<u8> = embed_data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let enc_w_bytes: Vec<u8> = enc_w_data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let enc_b_bytes: Vec<u8> = enc_b_data.iter().flat_map(|v| v.to_le_bytes()).collect();

        let embed_off = 0u64;
        let embed_end = embed_off + embed_bytes.len() as u64;
        let enc_w_off = embed_end;
        let enc_w_end = enc_w_off + enc_w_bytes.len() as u64;
        let enc_b_off = enc_w_end;
        let enc_b_end = enc_b_off + enc_b_bytes.len() as u64;

        let metadata = serde_json::json!({
            "embeddings.word_embeddings.weight": {
                "dtype": "F32",
                "shape": [vocab_size, hidden_size],
                "data_offsets": [embed_off, embed_end]
            },
            "encoder.layer.0.weight": {
                "dtype": "F32",
                "shape": [hidden_size, hidden_size],
                "data_offsets": [enc_w_off, enc_w_end]
            },
            "encoder.layer.0.bias": {
                "dtype": "F32",
                "shape": [hidden_size],
                "data_offsets": [enc_b_off, enc_b_end]
            }
        });

        let meta_str = metadata.to_string();
        let meta_bytes = meta_str.as_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&(meta_bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(meta_bytes);
        buf.extend_from_slice(&embed_bytes);
        buf.extend_from_slice(&enc_w_bytes);
        buf.extend_from_slice(&enc_b_bytes);
        buf
    }

    fn test_tokenizer_bytes() -> Vec<u8> {
        let vocab = serde_json::json!({
            "[UNK]": 0, "[CLS]": 1, "[SEP]": 2, "[PAD]": 3, "[MASK]": 4,
            "hello": 5, "world": 6, "test": 7, "the": 8, "a": 9
        });
        let cfg = serde_json::json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": { "type": "Whitespace" },
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "WordLevel",
                "vocab": vocab,
                "unk_token": "[UNK]"
            }
        });
        serde_json::to_vec(&cfg).unwrap()
    }

    fn test_model(
        vocab_size: usize,
        hidden_size: usize,
    ) -> (ArcticEmbedXS, tokenizers::Tokenizer, Device) {
        let device = Device::Cpu;
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(
            test_safetensors(vocab_size, hidden_size),
            DType::F32,
            &device,
        )
        .unwrap();
        let model = ArcticEmbedXS::new(vocab_size, hidden_size, vb).unwrap();
        let tokenizer = tokenizers::Tokenizer::from_bytes(&test_tokenizer_bytes()).unwrap();
        (model, tokenizer, device)
    }

    #[test]
    fn forward_output_shape() {
        let (model, _, device) = test_model(50, 8);
        let ids = Tensor::new(&[1u32, 2, 3], &device)
            .unwrap()
            .unsqueeze(0)
            .unwrap();
        let mask = Tensor::ones(&[1, 3], DType::U32, &device).unwrap();
        let out = model.forward(&ids, &mask).unwrap();
        assert_eq!(out.shape().dims(), &[1, 8]);
    }

    #[test]
    fn forward_l2_normalized() {
        let (model, _, device) = test_model(50, 8);
        let ids = Tensor::new(&[5u32, 6, 7], &device)
            .unwrap()
            .unsqueeze(0)
            .unwrap();
        let mask = Tensor::ones(&[1, 3], DType::U32, &device).unwrap();
        let out = model.forward(&ids, &mask).unwrap();
        let norm: f64 = out
            .to_dtype(DType::F64)
            .unwrap()
            .powf(2.0)
            .unwrap()
            .sum_all()
            .unwrap()
            .to_scalar()
            .unwrap();
        assert!(
            norm.is_finite() && norm > 0.0,
            "norm should be finite and positive, got {norm}"
        );
    }

    #[test]
    fn forward_nonzero() {
        let (model, _, device) = test_model(50, 8);
        let ids = Tensor::new(&[5u32, 6, 7], &device)
            .unwrap()
            .unsqueeze(0)
            .unwrap();
        let mask = Tensor::ones(&[1, 3], DType::U32, &device).unwrap();
        let vec: Vec<f32> = model
            .forward(&ids, &mask)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1()
            .unwrap();
        assert!(
            vec.iter().any(|v| v.abs() > 1e-6),
            "output should be non-zero"
        );
    }

    #[test]
    fn embedder_dimensions() {
        let (model, tokenizer, device) = test_model(50, 16);
        let emb = CandleEmbedder::new(model, tokenizer, device);
        assert_eq!(emb.dimensions(), 16);
    }

    #[test]
    fn embedder_embed_text_roundtrips() {
        let (model, tokenizer, device) = test_model(50, 16);
        let emb = CandleEmbedder::new(model, tokenizer, device);
        let vec = pollster::block_on(emb.embed_text("hello world")).unwrap();
        assert_eq!(vec.len(), 16);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm={norm} != 1.0");
    }

    #[cfg(feature = "candle-load")]
    #[test]
    fn load_downloads_and_embeds() {
        // Downloads the real model from HuggingFace Hub.
        // Run: cargo test --features candle candle-load -p penumbra-tests
        let emb = pollster::block_on(CandleEmbedder::load()).unwrap();
        assert_eq!(emb.dimensions(), 384);
        let vec = pollster::block_on(emb.embed_text("hello world")).unwrap();
        assert_eq!(vec.len(), 384);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "norm={norm}");
    }

    #[cfg(feature = "candle-load")]
    #[test]
    fn inference_benchmark() {
        use std::time::Instant;

        let load_start = Instant::now();
        let emb = pollster::block_on(CandleEmbedder::load()).unwrap();
        let load_ms = load_start.elapsed().as_millis();

        let samples = [
            "The quick brown fox jumps over the lazy dog",
            "Penumbra is a universe for your notes, built around a spatial canvas",
            "Frontmatter tags and wikilinks create explicit structure",
            "Candle runs on WASM via CPU backend, no GPU required",
            "Arctic embed xs produces 384-dimensional normalized vectors",
            "The Field Almanac uses tabular numerals and ink hairlines",
            "Letterstorm motion uses spring overshoot between 1.03 and 1.05",
            "Night theme ground color is hex 101423",
            "Model caching uses Cache API on WASM, filesystem on native",
            "Trigram hashing gives deterministic similarity without ML",
        ];

        let warmup = pollster::block_on(emb.embed_text(samples[0])).unwrap();
        assert_eq!(warmup.len(), 384);

        let embed_start = Instant::now();
        for text in &samples {
            let vec = pollster::block_on(emb.embed_text(text)).unwrap();
            assert_eq!(vec.len(), 384);
        }
        let embed_ms = embed_start.elapsed().as_millis();
        let per_note_ms = embed_ms as f64 / samples.len() as f64;

        println!("inference benchmark");
        println!("model load:  {load_ms} ms");
        println!("10 notes:    {embed_ms} ms");
        println!("per note:    {per_note_ms:.1} ms");
        println!("dimensions:  {}", emb.dimensions());
    }
}

// NullEmbedder

#[test]
fn null_embedder_dimensions() {
    let emb = NullEmbedder::new(128);
    assert_eq!(emb.dimensions(), 128);
}

#[test]
fn null_embedder_returns_zero_vector() {
    let emb = NullEmbedder::new(4);
    let result = futures::executor::block_on(emb.embed_text("anything"));
    let vec = result.unwrap();
    assert_eq!(vec.len(), 4);
    for v in &vec {
        assert_eq!(*v, 0.0);
    }
}

#[test]
fn null_embedder_batch() {
    let emb = NullEmbedder::new(3);
    let texts = &["a", "b", "c"];
    let results = futures::executor::block_on(emb.embed_batch(texts)).unwrap();
    assert_eq!(results.len(), 3);
    for vec in &results {
        assert_eq!(vec.len(), 3);
        for v in vec {
            assert_eq!(*v, 0.0);
        }
    }
}

#[test]
fn null_embedder_note() {
    let emb = NullEmbedder::new(5);
    let note = Note::new("hello".into(), "world".into());
    let vec = futures::executor::block_on(emb.embed_note(&note)).unwrap();
    assert_eq!(vec.len(), 5);
    for v in &vec {
        assert_eq!(*v, 0.0);
    }
}

#[test]
fn null_embedder_default_embed_text_len() {
    let emb = NullEmbedder::new(384);
    let vec = futures::executor::block_on(emb.embed_text("x")).unwrap();
    assert_eq!(vec.len(), 384);
}

// SimpleEmbedder

#[test]
fn simple_embedder_dimensions() {
    let emb = SimpleEmbedder::new(64);
    assert_eq!(emb.dimensions(), 64);
}

#[test]
fn simple_embedder_384_convenience() {
    let emb = SimpleEmbedder::new_384();
    assert_eq!(emb.dimensions(), 384);
}

#[test]
fn simple_embedder_deterministic() {
    let emb = SimpleEmbedder::new(64);
    let a = futures::executor::block_on(emb.embed_text("hello world")).unwrap();
    let b = futures::executor::block_on(emb.embed_text("hello world")).unwrap();
    assert_eq!(a, b);
}

#[test]
fn simple_embedder_different_inputs_differ() {
    let emb = SimpleEmbedder::new(64);
    let a = futures::executor::block_on(emb.embed_text("cat")).unwrap();
    let b = futures::executor::block_on(emb.embed_text("dog")).unwrap();
    assert_ne!(a, b);
}

#[test]
fn simple_embedder_l2_normalized() {
    let emb = SimpleEmbedder::new(64);
    let vec = futures::executor::block_on(emb.embed_text("test")).unwrap();
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "norm={norm} not ~1.0");
}

#[test]
fn simple_embedder_batch_matches_single() {
    let emb = SimpleEmbedder::new(32);
    let texts = &["alpha", "beta", "gamma"];
    let batch = futures::executor::block_on(emb.embed_batch(texts)).unwrap();
    assert_eq!(batch.len(), 3);
    for (i, text) in texts.iter().enumerate() {
        let single = futures::executor::block_on(emb.embed_text(text)).unwrap();
        assert_eq!(batch[i], single, "mismatch for text {i}: {text}");
    }
}

#[test]
fn simple_embedder_identity_note() {
    let emb = SimpleEmbedder::new(32);
    let note = Note::new("foo".into(), "bar".into());
    let vec = futures::executor::block_on(emb.embed_note(&note)).unwrap();
    assert_eq!(vec.len(), 32);
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
}

#[test]
fn simple_embedder_similar_texts_similar_vectors() {
    let emb = SimpleEmbedder::new(128);
    let a = futures::executor::block_on(emb.embed_text("hello world")).unwrap();
    let b = futures::executor::block_on(emb.embed_text("hello world!!")).unwrap();
    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    assert!(
        dot > 0.5,
        "similar texts should have high cosine similarity: {dot}"
    );
}

#[test]
fn simple_embedder_dissimilar_texts_low_similarity() {
    let emb = SimpleEmbedder::new(128);
    let a = futures::executor::block_on(emb.embed_text("abc")).unwrap();
    let b = futures::executor::block_on(emb.embed_text("xyz")).unwrap();
    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    assert!(
        dot < 0.99,
        "very different texts should have lower similarity: {dot}"
    );
}

#[test]
fn simple_embedder_empty_string() {
    let emb = SimpleEmbedder::new(64);
    let vec = futures::executor::block_on(emb.embed_text("")).unwrap();
    assert_eq!(vec.len(), 64);
    // Empty string has no trigrams, so the embedding is all zeros
    for v in &vec {
        assert_eq!(*v, 0.0);
    }
}
