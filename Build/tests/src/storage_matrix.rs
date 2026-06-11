use std::collections::HashMap;

use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_storage::Storage;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

#[test]
fn new_storage_succeeds() {
    runtime().block_on(async {
        let storage = Storage::new().await;
        assert!(storage.is_ok());
    });
}

#[test]
fn save_and_load_note_roundtrip() {
    runtime().block_on(async {
        let storage = Storage::new().await.unwrap();
        let note = Note::new("roundtrip title".into(), "roundtrip body".into());
        let id = note.id;

        storage.save_note(&note).await.unwrap();
        let loaded = storage.load_note(&id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.title, "roundtrip title");
        assert_eq!(loaded.body, "roundtrip body");

        // Cleanup
        storage.delete_note(&id).await.unwrap();
        let after_delete = storage.load_note(&id).await.unwrap();
        assert!(after_delete.is_none());
    });
}

#[test]
fn save_and_load_graph() {
    runtime().block_on(async {
        let storage = Storage::new().await.unwrap();
        let a = Note::new("A".into(), "".into());
        let b = Note::new("B".into(), "".into());
        let link = Link::new(a.id, b.id, LinkKind::Explicit);
        let notes = vec![a.clone(), b.clone()];
        let links = vec![link.clone()];

        storage.save_graph(&notes, &links).await.unwrap();
        let loaded = storage.load_graph().await.unwrap();
        assert!(loaded.is_some());
        let (loaded_notes, loaded_links) = loaded.unwrap();
        assert_eq!(loaded_notes.len(), 2);
        assert_eq!(loaded_links.len(), 1);
        assert_eq!(loaded_links[0], link);
    });
}

#[test]
fn save_and_load_positions() {
    runtime().block_on(async {
        let storage = Storage::new().await.unwrap();
        let id = NoteId::new();
        let mut positions = HashMap::new();
        positions.insert(id, Position::new(10.0, 20.0));

        storage.save_positions(&positions).await.unwrap();
        let loaded = storage.load_positions().await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.len(), 1);
        let pos = loaded.get(&id).unwrap();
        assert!((pos.x - 10.0).abs() < 1e-10);
        assert!((pos.y - 20.0).abs() < 1e-10);
    });
}

#[test]
fn load_nonexistent_note_returns_none() {
    runtime().block_on(async {
        let storage = Storage::new().await.unwrap();
        let result = storage.load_note(&NoteId::new()).await.unwrap();
        assert!(result.is_none());
    });
}
