use penumbra_core::embed::EmbeddingProvider;
use penumbra_core::note::Note;
use penumbra_embed::NullEmbedder;

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
