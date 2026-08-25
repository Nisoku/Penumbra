use std::future::Future;

use opfs::persistent::DirectoryHandle;
use penumbra_app::Universe;
use penumbra_storage::Storage;

fn with_rt(f: impl Future<Output = ()>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(f);
}

fn temp_storage() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = DirectoryHandle::from(dir.path().join("universe"));
    let storage = pollster::block_on(Storage::with_dir(root));
    (dir, storage)
}

#[test]
fn empty_universe_opens_with_zero_notes() {
    with_rt(async {
        let (_guard, storage) = temp_storage();
        let universe = Universe::open(storage).await.unwrap();
        assert_eq!(universe.note_count(), 0);
    });
}

#[test]
fn created_note_survives_universe_reopen() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let id = universe
            .create_note("Alpha".to_string(), "first body".to_string())
            .await
            .unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("universe"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(universe.note_count(), 1);
        assert_eq!(universe.graph().get_note(&id).unwrap().title, "Alpha");
    });
}

#[test]
fn saved_body_edit_persists_across_reopen() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let id = universe
            .create_note("Beta".to_string(), String::new())
            .await
            .unwrap();

        let mut edited = universe.graph().get_note(&id).unwrap().clone();
        edited.body = "rewritten body".to_string();
        universe.save_note(edited).await.unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("universe"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(
            universe.graph().get_note(&id).unwrap().body,
            "rewritten body"
        );
    });
}

#[test]
fn deleted_note_is_gone_after_reopen() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let id = universe
            .create_note("Gamma".to_string(), "doomed".to_string())
            .await
            .unwrap();
        universe.delete_note(&id).await.unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("universe"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(universe.note_count(), 0);
        assert!(universe.graph().get_note(&id).is_none());
    });
}
