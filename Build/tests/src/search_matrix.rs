use std::sync::{Arc, Mutex};

use penumbra_core::error::Result;
use penumbra_core::note::Note;
use penumbra_embed::NullEmbedder;
use penumbra_index::RuvectorIndex;
use penumbra_index::VectorIndex;
use penumbra_search::{SearchConfig, SearchEngine};

fn make_index(dims: usize) -> Arc<Mutex<dyn VectorIndex>> {
    Arc::new(Mutex::new(RuvectorIndex::new(dims).unwrap()))
}

fn make_engine_with_config(dims: usize, config: SearchConfig) -> SearchEngine {
    let embedder = Arc::new(NullEmbedder::new(dims));
    let index = make_index(dims);
    SearchEngine::with_config(embedder, index, config)
}

fn make_engine(dims: usize) -> SearchEngine {
    let embedder = Arc::new(NullEmbedder::new(dims));
    let index = make_index(dims);
    SearchEngine::new(embedder, index)
}

fn insert_note(engine: &SearchEngine, note: Note) -> Result<()> {
    let id = note.id;
    let vector = futures::executor::block_on(engine.embedder().embed_note(&note))?;
    let mut idx = engine.index().lock().unwrap();
    idx.insert(id, &vector)
}

fn search(
    engine: &SearchEngine,
    query: &str,
    notes: &[Note],
    tags: &[String],
) -> Vec<penumbra_search::SearchResult> {
    futures::executor::block_on(engine.search(query, notes, tags)).unwrap()
}

#[test]
fn new_engine_default_config() {
    let engine = make_engine(4);
    let config = engine.config();
    assert!((config.vector_weight - 1.0).abs() < 1e-10);
}

#[test]
fn search_returns_results() {
    let engine = make_engine(4);
    let note = Note::new("test".into(), "body".into());
    let notes = vec![note.clone()];
    insert_note(&engine, note).unwrap();
    let results = search(&engine, "test", &notes, &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note.title, "test");
}

#[test]
fn search_empty_index_returns_empty() {
    let engine = make_engine(4);
    let results = search(&engine, "anything", &[], &[]);
    assert!(results.is_empty());
}

#[test]
fn search_respects_max_results() {
    let config = SearchConfig {
        max_results: 3,
        ..Default::default()
    };
    let engine = make_engine_with_config(4, config);
    let mut notes = Vec::new();
    for i in 0..10 {
        let note = Note::new(format!("title {i}"), format!("body {i}"));
        insert_note(&engine, note.clone()).unwrap();
        notes.push(note);
    }
    let results = search(&engine, "title", &notes, &[]);
    assert_eq!(results.len(), 3);
}

#[test]
fn results_sorted_by_score_descending() {
    let engine = make_engine(4);
    let mut notes = Vec::new();
    for i in 0..5 {
        let note = Note::new(format!("title {i}"), "common body".into());
        insert_note(&engine, note.clone()).unwrap();
        notes.push(note);
    }
    let results = search(&engine, "title 0", &notes, &[]);
    assert!(!results.is_empty());
    for w in results.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "results not sorted: {:?}",
            (w[0].score, w[1].score)
        );
    }
}

#[test]
fn tag_filter_only_returns_matching_notes() {
    let engine = make_engine(4);
    let mut note_a = Note::new("alpha".into(), "".into());
    note_a.tags = vec!["urgent".to_string()];
    let note_b = Note::new("beta".into(), "".into());
    let notes = vec![note_a.clone(), note_b.clone()];
    insert_note(&engine, note_a.clone()).unwrap();
    insert_note(&engine, note_b).unwrap();
    // Tag filter excludes notes without the tag
    let results = search(&engine, "", &notes, &["urgent".to_string()]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note.title, "alpha");
    assert!(results[0].tag_score > 0.0);
}

#[test]
fn tag_score_positive_for_matching_notes() {
    let engine = make_engine(4);
    let mut note = Note::new("tagged".into(), "".into());
    note.tags = vec!["important".to_string(), "work".to_string()];
    let notes = vec![note.clone()];
    insert_note(&engine, note).unwrap();
    let results = search(&engine, "tagged", &notes, &["important".to_string()]);
    assert_eq!(results.len(), 1);
    // tag_score should be 1.0 (1/1 tags matched)
    assert!((results[0].tag_score - 1.0).abs() < 1e-10);
}

#[test]
fn text_relevance_boosts_title_matches() {
    let engine = make_engine(4);
    // With null embedder, vector similarity is 0 for everything
    // So text relevance is the primary signal
    let note_a = Note::new("specific term".into(), "irrelevant body".into());
    let note_b = Note::new("unrelated".into(), "specific term in body".into());
    let notes = vec![note_a.clone(), note_b.clone()];
    insert_note(&engine, note_a.clone()).unwrap();
    insert_note(&engine, note_b).unwrap();
    let results = search(&engine, "specific term", &notes, &[]);
    assert_eq!(results.len(), 2);
    // Title match should score higher than body-only match
    assert_eq!(results[0].note.title, "specific term");
    assert!(results[0].text_score > results[1].text_score);
}

#[test]
fn temporal_decay_prefers_recent() {
    use chrono::{TimeDelta, Utc};
    let engine = make_engine(4);
    let now = Utc::now();

    let mut old = Note::new("old".into(), "".into());
    old.meta.updated_at = now - TimeDelta::try_days(300).unwrap();

    let mut recent = Note::new("recent".into(), "".into());
    recent.meta.updated_at = now;

    let notes = vec![old.clone(), recent.clone()];
    insert_note(&engine, old).unwrap();
    insert_note(&engine, recent).unwrap();

    let results = search(&engine, "anything", &notes, &[]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].note.title, "recent");
    assert!(results[0].temporal_score > results[1].temporal_score);
}

#[test]
fn search_does_not_panic_on_empty_query() {
    let engine = make_engine(4);
    let note = Note::new("title".into(), "body".into());
    let notes = vec![note.clone()];
    insert_note(&engine, note).unwrap();
    let results = search(&engine, "", &notes, &[]);
    // Empty query with no tags returns empty early
    assert!(results.is_empty());
}

#[test]
fn search_empty_query_with_tags_still_searches() {
    let engine = make_engine(4);
    let mut note = Note::new("tagged".into(), "".into());
    note.tags = vec!["important".to_string()];
    let notes = vec![note.clone()];
    insert_note(&engine, note).unwrap();
    let results = search(&engine, "", &notes, &["important".to_string()]);
    // Should return results based on tag + temporal alone
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note.title, "tagged");
}

#[test]
fn search_multiple_notes_with_same_vector() {
    let engine = make_engine(4);
    let mut notes = Vec::new();
    for i in 0..5 {
        let note = Note::new(format!("note {i}"), "".into());
        insert_note(&engine, note.clone()).unwrap();
        notes.push(note);
    }
    let results = search(&engine, "anything", &notes, &[]);
    assert_eq!(results.len(), 5);
}
