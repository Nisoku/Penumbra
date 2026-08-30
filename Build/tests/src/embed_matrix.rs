use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::note::Note;
use penumbra_embed::{NullEmbedder, SimpleEmbedder};

#[cfg(feature = "candle")]
mod candle {
    use candle_core::{DType, Device, Tensor};
    use penumbra_core::embed::EmbeddingProvider;
    use penumbra_embed::candle::{ArcticEmbedXS, CandleEmbedder};

    const TEST_VOCAB: usize = 10;
    const TEST_HIDDEN: usize = 8;
    const TEST_NUM_LAYERS: usize = 1;
    const TEST_NUM_HEADS: usize = 2;

    fn test_config_bytes() -> Vec<u8> {
        let cfg = serde_json::json!({
            "hidden_size": TEST_HIDDEN,
            "num_hidden_layers": TEST_NUM_LAYERS,
            "num_attention_heads": TEST_NUM_HEADS,
            "intermediate_size": TEST_HIDDEN * 2,
            "vocab_size": TEST_VOCAB,
            "max_position_embeddings": 32,
            "type_vocab_size": 2,
            "hidden_act": "gelu",
            "hidden_dropout_prob": 0.0,
            "attention_probs_dropout_prob": 0.0,
            "layer_norm_eps": 1e-12,
            "pad_token_id": 0,
            "initializer_range": 0.02,
            "classifier_dropout": null,
            "position_embedding_type": "absolute",
            "use_cache": false,
            "model_type": "bert",
        });
        serde_json::to_vec(&cfg).unwrap()
    }

    fn test_safetensors() -> Vec<u8> {
        let h = TEST_HIDDEN;
        let v = TEST_VOCAB;
        let n = TEST_NUM_LAYERS;

        let f32s = |count: usize| -> Vec<u8> {
            (0..count)
                .flat_map(|i| ((i as f32 * 0.01).fract()).to_le_bytes())
                .collect()
        };

        let mut tensors = vec![
            (
                "bert.embeddings.word_embeddings.weight".into(),
                f32s(v * h),
                vec![v, h],
            ),
            (
                "bert.embeddings.position_embeddings.weight".into(),
                f32s(32 * h),
                vec![32, h],
            ),
            (
                "bert.embeddings.token_type_embeddings.weight".into(),
                f32s(2 * h),
                vec![2, h],
            ),
            ("bert.embeddings.LayerNorm.weight".into(), f32s(h), vec![h]),
            ("bert.embeddings.LayerNorm.bias".into(), f32s(h), vec![h]),
        ];

        for i in 0..n {
            let prefix = format!("bert.encoder.layer.{i}");
            tensors.push((
                format!("{prefix}.attention.self.query.weight"),
                f32s(h * h),
                vec![h, h],
            ));
            tensors.push((
                format!("{prefix}.attention.self.query.bias"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.attention.self.key.weight"),
                f32s(h * h),
                vec![h, h],
            ));
            tensors.push((
                format!("{prefix}.attention.self.key.bias"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.attention.self.value.weight"),
                f32s(h * h),
                vec![h, h],
            ));
            tensors.push((
                format!("{prefix}.attention.self.value.bias"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.attention.output.dense.weight"),
                f32s(h * h),
                vec![h, h],
            ));
            tensors.push((
                format!("{prefix}.attention.output.dense.bias"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.attention.output.LayerNorm.weight"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.attention.output.LayerNorm.bias"),
                f32s(h),
                vec![h],
            ));
            tensors.push((
                format!("{prefix}.intermediate.dense.weight"),
                f32s(h * h * 2),
                vec![h * 2, h],
            ));
            tensors.push((
                format!("{prefix}.intermediate.dense.bias"),
                f32s(h * 2),
                vec![h * 2],
            ));
            tensors.push((
                format!("{prefix}.output.dense.weight"),
                f32s(h * h * 2),
                vec![h, h * 2],
            ));
            tensors.push((format!("{prefix}.output.dense.bias"), f32s(h), vec![h]));
            tensors.push((
                format!("{prefix}.output.LayerNorm.weight"),
                f32s(h),
                vec![h],
            ));
            tensors.push((format!("{prefix}.output.LayerNorm.bias"), f32s(h), vec![h]));
        }

        tensors.push(("bert.pooler.dense.weight".into(), f32s(h * h), vec![h, h]));
        tensors.push(("bert.pooler.dense.bias".into(), f32s(h), vec![h]));

        let mut offsets: Vec<(String, u64, u64, Vec<usize>)> = Vec::new();
        let mut cursor: u64 = 0;
        let mut data_sections: Vec<Vec<u8>> = Vec::new();

        for (name, bytes, shape) in &tensors {
            let off = cursor;
            let end = cursor + bytes.len() as u64;
            offsets.push((name.clone(), off, end, shape.clone()));
            data_sections.push(bytes.clone());
            cursor = end;
        }

        let mut metadata = serde_json::Map::new();
        for (name, off, end, shape) in &offsets {
            metadata.insert(
                name.clone(),
                serde_json::json!({
                    "dtype": "F32",
                    "shape": shape,
                    "data_offsets": [off, end]
                }),
            );
        }

        let meta_str = serde_json::Value::Object(metadata).to_string();
        let meta_bytes = meta_str.as_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&(meta_bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(meta_bytes);
        for data in &data_sections {
            buf.extend_from_slice(data);
        }
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

    fn test_model() -> (ArcticEmbedXS, tokenizers::Tokenizer, Device) {
        let device = Device::Cpu;
        let config = test_config_bytes();
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(
            test_safetensors(),
            DType::F32,
            &device,
        )
        .unwrap();
        let model = ArcticEmbedXS::new(&config, vb).unwrap();
        let tokenizer = tokenizers::Tokenizer::from_bytes(test_tokenizer_bytes()).unwrap();
        (model, tokenizer, device)
    }

    #[test]
    fn forward_output_shape() {
        let (model, _, device) = test_model();
        let ids = Tensor::new(&[1u32, 2, 3], &device)
            .unwrap()
            .unsqueeze(0)
            .unwrap();
        let mask = Tensor::ones(&[1, 3], DType::U32, &device).unwrap();
        let out = model.forward(&ids, &mask).unwrap();
        assert_eq!(out.shape().dims(), &[1, TEST_HIDDEN]);
    }

    #[test]
    fn forward_l2_normalized() {
        let (model, _, device) = test_model();
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
        let (model, _, device) = test_model();
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
        let (model, tokenizer, _device) = test_model();
        let emb = CandleEmbedder::new(model, tokenizer, Device::Cpu);
        assert_eq!(emb.dimensions(), TEST_HIDDEN);
    }

    #[test]
    fn embedder_embed_text_roundtrips() {
        let (model, tokenizer, _device) = test_model();
        let emb = CandleEmbedder::new(model, tokenizer, Device::Cpu);
        let vec = pollster::block_on(emb.embed_text("hello world")).unwrap();
        assert_eq!(vec.len(), TEST_HIDDEN);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm={norm} != 1.0");
    }

    #[cfg(feature = "candle-load")]
    #[tokio::test]
    async fn load_downloads_and_embeds() {
        let emb = CandleEmbedder::load().await.unwrap();
        assert_eq!(emb.dimensions(), 384);
        let vec = emb.embed_text("hello world").await.unwrap();
        assert_eq!(vec.len(), 384);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "norm={norm}");
    }

    #[cfg(feature = "candle-load")]
    #[tokio::test]
    async fn inference_benchmark() {
        use std::time::Instant;

        let load_start = Instant::now();
        let emb = CandleEmbedder::load().await.unwrap();
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

        let warmup = emb.embed_text(samples[0]).await.unwrap();
        assert_eq!(warmup.len(), 384);

        let embed_start = Instant::now();
        for text in &samples {
            let vec = emb.embed_text(text).await.unwrap();
            assert_eq!(vec.len(), 384);
        }
        let embed_ms = embed_start.elapsed().as_millis();
        let per_note_ms = embed_ms as f64 / samples.len() as f64;

        println!("--- inference benchmark ---");
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
fn simple_embedder_fn_buckets_are_stable_across_runs() {
    // Golden snapshot: guards against regressing to a per-process random hash
    // (e.g. `DefaultHasher`), which would silently change every embedding and
    // invalidate a persisted index.
    let emb = SimpleEmbedder::new(64);
    let vec = futures::executor::block_on(emb.embed_text("hello world")).unwrap();
    let expected = [13usize, 23, 24, 30, 36, 43, 46, 53, 56];
    let mut count = 0;
    for (i, v) in vec.iter().enumerate() {
        if expected.contains(&i) {
            assert!(
                (v - 1.0 / 3.0).abs() < 1e-5,
                "bucket {i} unexpected value {v}"
            );
            count += 1;
        } else {
            assert_eq!(*v, 0.0, "expected empty bucket {i}, got {v}");
        }
    }
    assert_eq!(count, expected.len());
}

#[test]
fn simple_embedder_unicode_deterministic() {
    let emb = SimpleEmbedder::new(64);
    let text = "héllo wörld 你好世界";
    let a = futures::executor::block_on(emb.embed_text(text)).unwrap();
    let b = futures::executor::block_on(emb.embed_text(text)).unwrap();
    assert_eq!(a, b);
    assert!(
        a.iter().any(|v| *v > 0.0),
        "unicode text should map to non-zero buckets"
    );
    let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "norm={norm} not ~1.0");
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
    for v in &vec {
        assert_eq!(*v, 0.0);
    }
}

#[test]
fn simple_embedder_zero_dims_clamped() {
    let emb = SimpleEmbedder::new(0);
    assert_eq!(emb.dimensions(), 1);
    let vec = futures::executor::block_on(emb.embed_text("x")).unwrap();
    assert_eq!(vec.len(), 1);
}
