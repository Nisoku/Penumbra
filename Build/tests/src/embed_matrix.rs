use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::note::Note;
use penumbra_embed::{NullEmbedder, SimpleEmbedder};

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
