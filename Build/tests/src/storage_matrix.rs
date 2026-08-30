use std::collections::HashMap;

use opfs::persistent::DirectoryHandle;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_storage::Storage;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

async fn temp_storage() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("vault");
    std::fs::create_dir_all(&root).unwrap();
    let storage = Storage::with_dir(DirectoryHandle::from(root)).await;
    (dir, storage)
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
        let (_guard, storage) = temp_storage().await;
        let note = Note::new("roundtrip title".into(), "roundtrip body\n".into());
        let id = note.id;

        let stem = storage.save_note(&note, &[]).await.unwrap();
        assert_eq!(stem, "roundtrip title");

        let stored = storage.scan().await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].note.id, id);
        assert_eq!(stored[0].note.title, "roundtrip title");
        assert_eq!(stored[0].note.body, "roundtrip body\n");
        assert_eq!(storage.filename_of(&id), Some("roundtrip title".into()));

        storage.delete_note(&id).await.unwrap();
        let after = storage.scan().await.unwrap();
        assert!(after.is_empty());
    });
}

#[test]
fn save_and_load_positions() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
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
fn nonexistent_note_has_no_filename() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        assert!(storage.filename_of(&NoteId::new()).is_none());
    });
}

#[test]
fn scan_empty_vault_returns_empty() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let stored = storage.scan().await.unwrap();
        assert!(stored.is_empty());
    });
}

#[test]
fn title_change_triggers_file_rename() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let note = Note::new("Old Title".into(), "body\n".into());
        let id = note.id;
        storage.save_note(&note, &[]).await.unwrap();
        assert_eq!(storage.filename_of(&id), Some("Old Title".into()));

        let mut renamed = note;
        renamed.title = "New Title".into();
        renamed.touch();
        storage.save_note(&renamed, &[]).await.unwrap();
        assert_eq!(storage.filename_of(&id), Some("New Title".into()));

        let stored = storage.scan().await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].note.title, "New Title");
    });
}

#[test]
fn duplicate_title_gets_deduped_stem() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let a = Note::new("Same Name".into(), "a\n".into());
        let b = Note::new("Same Name".into(), "b\n".into());
        storage.save_note(&a, &[]).await.unwrap();
        storage.save_note(&b, &[]).await.unwrap();

        let stored = storage.scan().await.unwrap();
        assert_eq!(stored.len(), 2);
        let stems: Vec<&str> = stored.iter().map(|s| s.filename.as_str()).collect();
        assert!(stems.contains(&"Same Name"));
        assert!(stems.contains(&"Same Name 2"));
    });
}

#[test]
fn implicit_links_roundtrip() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let a = Note::new("alpha".into(), "body\n".into());
        let b = Note::new("beta".into(), "body\n".into());
        let pairs = [(a.id, b.id), (b.id, a.id)].into_iter().collect();

        storage.save_implicit_links(&pairs).await.unwrap();
        let loaded = storage.load_implicit_links().await.unwrap().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains(&(a.id, b.id)));
        assert!(loaded.contains(&(b.id, a.id)));
    });
}

#[test]
fn implicit_links_empty_when_none_saved() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let loaded = storage
            .load_implicit_links()
            .await
            .unwrap()
            .unwrap_or_default();
        assert!(loaded.is_empty());
    });
}

#[test]
fn implicit_links_roundtrip_empty_set() {
    runtime().block_on(async {
        let (_guard, storage) = temp_storage().await;
        let empty = std::collections::HashSet::new();
        storage.save_implicit_links(&empty).await.unwrap();
        let loaded = storage.load_implicit_links().await.unwrap_or_default();
        assert_eq!(loaded, Some(empty));
    });
}
