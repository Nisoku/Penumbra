use penumbra_core::note::NoteId;
use penumbra_index::usearch_backend::USearchIndex;
use penumbra_index::VectorIndex;

fn make_vec(v: &[f32]) -> Vec<f32> {
    v.to_vec()
}

#[test]
fn new_index_empty() {
    let idx = USearchIndex::new(4).unwrap();
    assert_eq!(idx.len(), 0);
    assert!(idx.is_empty());
}

#[test]
fn insert_and_self_search() {
    let mut idx = USearchIndex::new(4).unwrap();
    let id = NoteId::new();
    idx.insert(id, &make_vec(&[1.0, 0.0, 0.0, 0.0])).unwrap();
    let hits = idx.search(&make_vec(&[1.0, 0.0, 0.0, 0.0]), 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, id);
    assert!((hits[0].score - 1.0).abs() < 1e-5);
}

#[test]
fn insert_replaces_existing() {
    let mut idx = USearchIndex::new(4).unwrap();
    let id = NoteId::new();
    idx.insert(id, &make_vec(&[1.0, 0.0, 0.0, 0.0])).unwrap();
    idx.insert(id, &make_vec(&[0.0, 1.0, 0.0, 0.0])).unwrap();
    assert_eq!(idx.len(), 1);
    let hits = idx.search(&make_vec(&[0.0, 1.0, 0.0, 0.0]), 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, id);
}

#[test]
fn remove_entry() {
    let mut idx = USearchIndex::new(4).unwrap();
    let id = NoteId::new();
    idx.insert(id, &make_vec(&[1.0, 0.0, 0.0, 0.0])).unwrap();
    idx.remove(&id).unwrap();
    assert_eq!(idx.len(), 0);
    let hits = idx.search(&make_vec(&[1.0, 0.0, 0.0, 0.0]), 5).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn remove_nonexistent_is_noop() {
    let mut idx = USearchIndex::new(4).unwrap();
    idx.remove(&NoteId::new()).unwrap();
    assert_eq!(idx.len(), 0);
}

#[test]
fn search_empty_returns_empty() {
    let idx = USearchIndex::new(4).unwrap();
    let hits = idx.search(&make_vec(&[1.0, 0.0, 0.0, 0.0]), 5).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn search_returns_k_results() {
    let mut idx = USearchIndex::new(8).unwrap();
    let ids: Vec<NoteId> = (0..8).map(|_| NoteId::new()).collect();
    for (i, id) in ids.iter().enumerate() {
        let mut vec = vec![0.0f32; 8];
        vec[i] = 1.0;
        idx.insert(*id, &vec).unwrap();
    }
    let hits = idx
        .search(&make_vec(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]), 3)
        .unwrap();
    assert_eq!(hits.len(), 3);
    // First hit should be the first inserted vector (identical to query)
    assert_eq!(hits[0].id, ids[0]);
    assert!((hits[0].score - 1.0).abs() < 1e-5);
}

#[test]
fn dimension_mismatch_returns_error() {
    let mut idx = USearchIndex::new(4).unwrap();
    let err = idx
        .insert(NoteId::new(), &make_vec(&[1.0, 2.0, 3.0]))
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected 4"), "got: {msg}");
}

#[test]
fn cosine_similarity_ordered() {
    let mut idx = USearchIndex::new(4).unwrap();
    let id_identical = NoteId::new();
    let id_similar = NoteId::new();
    let id_different = NoteId::new();

    idx.insert(id_identical, &make_vec(&[1.0, 1.0, 0.0, 0.0]))
        .unwrap();
    idx.insert(id_similar, &make_vec(&[0.9, 0.9, 0.1, 0.0]))
        .unwrap();
    idx.insert(id_different, &make_vec(&[0.0, 0.0, 1.0, 1.0]))
        .unwrap();

    let hits = idx.search(&make_vec(&[1.0, 1.0, 0.0, 0.0]), 3).unwrap();
    assert_eq!(hits.len(), 3);
    // id_identical should be first (score ~1.0)
    assert_eq!(hits[0].id, id_identical);
    // id_different should be last (lowest similarity)
    assert_eq!(hits[2].id, id_different);
}

#[test]
fn many_inserts_stress() {
    let mut idx = USearchIndex::new(16).unwrap();
    let ids: Vec<NoteId> = (0..100).map(|_| NoteId::new()).collect();
    for (i, id) in ids.iter().enumerate() {
        let mut vec = vec![0.0f32; 16];
        vec[i % 16] = 1.0;
        idx.insert(*id, &vec).unwrap();
    }
    assert_eq!(idx.len(), 100);
    let hits = idx
        .search(
            &make_vec(&[
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ]),
            10,
        )
        .unwrap();
    assert_eq!(hits.len(), 10);
}
