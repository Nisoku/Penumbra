use penumbra_core::link::LinkKind;
use penumbra_core::note::Note;
use penumbra_graph::GraphStore;

#[test]
fn add_and_retrieve_note() {
    let mut store = GraphStore::new();
    let note = Note::new("Test".into(), "Body".into());
    let id = note.id;
    assert!(store.add_note(note));
    assert_eq!(store.note_count(), 1);
    assert!(store.get_note(&id).is_some());
}

#[test]
fn remove_note() {
    let mut store = GraphStore::new();
    let note = Note::new("Test".into(), "Body".into());
    let id = note.id;
    store.add_note(note);
    assert!(store.remove_note(&id).is_some());
    assert_eq!(store.note_count(), 0);
}

#[test]
fn link_and_unlink() {
    let mut store = GraphStore::new();
    let a = Note::new("A".into(), "".into());
    let b = Note::new("B".into(), "".into());
    let id_a = a.id;
    let id_b = b.id;
    store.add_note(a);
    store.add_note(b);

    let link = store.link_notes(&id_a, &id_b, LinkKind::Explicit).unwrap();
    assert_eq!(link.kind, LinkKind::Explicit);
    assert_eq!(store.link_count(), 1);
    assert_eq!(store.get_neighbors(&id_a).len(), 1);

    store.unlink_notes(&id_a, &id_b).unwrap();
    assert_eq!(store.link_count(), 0);
}

#[test]
fn connected_component() {
    let mut store = GraphStore::new();
    let a = Note::new("A".into(), "".into());
    let b = Note::new("B".into(), "".into());
    let c = Note::new("C".into(), "".into());
    let id_a = a.id;
    let id_b = b.id;
    let id_c = c.id;
    store.add_note(a);
    store.add_note(b);
    store.add_note(c);

    store.link_notes(&id_a, &id_b, LinkKind::Explicit).unwrap();

    let component = store.get_connected_component(&id_a);
    assert!(component.contains(&id_a));
    assert!(component.contains(&id_b));
    assert!(!component.contains(&id_c));
}

#[test]
fn snapshot_roundtrip() {
    let mut store = GraphStore::new();
    let a = Note::new("A".into(), "".into());
    let b = Note::new("B".into(), "".into());
    let id_a = a.id;
    let id_b = b.id;
    store.add_note(a);
    store.add_note(b);
    store.link_notes(&id_a, &id_b, LinkKind::Explicit).unwrap();

    let snapshot = store.snapshot();
    let mut restored = GraphStore::new();
    restored.restore(snapshot);

    assert_eq!(restored.note_count(), 2);
    assert_eq!(restored.link_count(), 1);
    assert!(restored.get_note(&id_a).is_some());
    assert!(restored.get_note(&id_b).is_some());
    assert_eq!(restored.get_neighbors(&id_a).len(), 1);
}
