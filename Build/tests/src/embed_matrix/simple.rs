use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::note::Note;
use penumbra_embed::SimpleEmbedder;

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
