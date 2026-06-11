use std::sync::{Arc, Mutex};

use penumbra_auto_link::{AutoLinkConfig, AutoLinker};
use penumbra_core::link::LinkKind;
use penumbra_core::EmbeddingProvider;
use penumbra_core::Note;
use penumbra_embed::SimpleEmbedder;
use penumbra_events::EventBus;
use penumbra_graph::GraphStore;
use penumbra_index::RuvectorIndex;
use penumbra_index::VectorIndex;

/// Body text that produces high-similarity trigram embeddings when shared
/// between two notes.
const IDENTICAL_BODY: &str = "\
    common repeating text that appears in every test note \
    so trigrams overlap heavily with any other note using \
    the same body";

fn make_note(title: &str, body: &str) -> Note {
    Note::new(title.to_string(), body.to_string())
}

fn setup() -> (AutoLinker, Note, Note) {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("alpha", IDENTICAL_BODY);
    let note_b = make_note("beta", IDENTICAL_BODY);

    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();

    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());

    let linker = AutoLinker::with_defaults(embedder, index, graph, bus);
    (linker, note_a, note_b)
}

#[test]
fn auto_link_config_defaults() {
    let cfg = AutoLinkConfig::default();
    assert!(cfg.top_k > 0);
    assert!(cfg.min_score > 0.0);
    assert!(cfg.max_links > 0);
}

#[test]
fn process_note_creates_implicit_link() {
    let (linker, note_a, _note_b) = setup();
    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();

    assert!(!links.is_empty(), "should have created at least one link");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].kind, LinkKind::Implicit);
}

#[test]
fn process_note_skips_self() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("alpha", IDENTICAL_BODY);
    let note_b = make_note("beta", IDENTICAL_BODY);

    // Pre-index both notes so the search can find note_b but should
    // not link note_a to itself.
    let emb_a = pollster::block_on(embedder.embed_note(&note_a)).unwrap();
    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    index.lock().unwrap().insert(note_a.id, &emb_a).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();

    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());

    let linker = AutoLinker::with_defaults(embedder, index, graph, bus);
    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();

    for l in &links {
        assert!(l.source != note_a.id || l.target != note_a.id);
    }
}

#[test]
fn process_note_obeys_score_threshold() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("alpha", IDENTICAL_BODY);
    let note_b = make_note("beta", IDENTICAL_BODY);

    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();
    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());

    let config = AutoLinkConfig {
        min_score: 1.01,
        ..Default::default()
    };
    let linker = AutoLinker::new(embedder, index, graph, bus, config);

    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();
    assert!(links.is_empty(), "no links should cross a 1.01 threshold");
}

#[test]
fn process_note_respects_max_links() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("one", IDENTICAL_BODY);
    let note_b = make_note("two", IDENTICAL_BODY);
    let note_c = make_note("three", IDENTICAL_BODY);

    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    let emb_c = pollster::block_on(embedder.embed_note(&note_c)).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();
    index.lock().unwrap().insert(note_c.id, &emb_c).unwrap();
    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());
    graph.lock().unwrap().add_note(note_c.clone());

    let config = AutoLinkConfig {
        max_links: 1,
        ..Default::default()
    };
    let linker = AutoLinker::new(embedder, index, graph, bus, config);

    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();
    assert_eq!(links.len(), 1, "should create at most 1 link");
}

#[test]
fn process_note_no_candidates_returns_empty() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note = make_note("lonely", "nobody in the index yet");
    graph.lock().unwrap().add_note(note.clone());

    let linker = AutoLinker::with_defaults(embedder, index, graph, bus);
    let links = pollster::block_on(linker.process_note(&note)).unwrap();
    assert!(links.is_empty(), "no candidates means no links");
}

#[test]
fn process_note_does_not_create_duplicates() {
    let (linker, note_a, _note_b) = setup();

    let first = pollster::block_on(linker.process_note(&note_a)).unwrap();
    assert_eq!(first.len(), 1);

    let second = pollster::block_on(linker.process_note(&note_a)).unwrap();
    assert!(
        second.is_empty(),
        "second pass should not create duplicate links"
    );
}

#[test]
fn process_note_empty_body_does_not_panic() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("empty", "");
    let note_b = make_note("also empty", "");
    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());

    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();

    let linker = AutoLinker::with_defaults(embedder, index, graph, bus);
    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();
    // Empty body may produce low-score embeddings; just check no panic.
    assert!(links.is_empty() || links.len() <= 5);
}

#[test]
fn process_note_top_k_zero_returns_empty() {
    let embedder = Arc::new(SimpleEmbedder::new_384());
    let index = Arc::new(Mutex::new(RuvectorIndex::new(384).unwrap()));
    let graph = Arc::new(Mutex::new(GraphStore::new()));
    let bus = Arc::new(EventBus::new());

    let note_a = make_note("alpha", IDENTICAL_BODY);
    let note_b = make_note("beta", IDENTICAL_BODY);
    graph.lock().unwrap().add_note(note_a.clone());
    graph.lock().unwrap().add_note(note_b.clone());

    let emb_b = pollster::block_on(embedder.embed_note(&note_b)).unwrap();
    index.lock().unwrap().insert(note_b.id, &emb_b).unwrap();

    let config = AutoLinkConfig {
        top_k: 0,
        ..Default::default()
    };
    let linker = AutoLinker::new(embedder, index, graph, bus, config);
    let links = pollster::block_on(linker.process_note(&note_a)).unwrap();
    assert!(links.is_empty(), "top_k=0 means no neighbours");
}
