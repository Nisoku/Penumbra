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
    let path = dir.path().join("vault");
    std::fs::create_dir_all(&path).unwrap();
    let root = DirectoryHandle::from(path);
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
            .create_note("Alpha".to_string(), "first body\n".to_string())
            .await
            .unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("vault"),
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
            .create_note("Beta".to_string(), "original\n".to_string())
            .await
            .unwrap();

        let mut edited = universe.graph().get_note(&id).unwrap().clone();
        edited.body = "rewritten body\n".to_string();
        edited.touch();
        universe.save_note(edited).await.unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("vault"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(
            universe.graph().get_note(&id).unwrap().body,
            "rewritten body\n"
        );
    });
}

#[test]
fn deleted_note_is_gone_after_reopen() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let id = universe
            .create_note("Gamma".to_string(), "doomed\n".to_string())
            .await
            .unwrap();
        universe.delete_note(&id).await.unwrap();
        assert_eq!(universe.note_count(), 0);
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("vault"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(universe.note_count(), 0);
        assert!(universe.graph().get_note(&id).is_none());
    });
}

#[test]
fn implicit_links_survive_universe_reopen() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let a = universe
            .create_note("Linked A".to_string(), "alpha\n".to_string())
            .await
            .unwrap();
        let b = universe
            .create_note("Linked B".to_string(), "beta\n".to_string())
            .await
            .unwrap();
        universe
            .graph()
            .link_notes(&a, &b, penumbra_core::link::LinkKind::Implicit)
            .unwrap();
        assert_eq!(universe.implicit_link_pairs().len(), 1);
        universe
            .storage()
            .save_implicit_links(&universe.implicit_link_pairs())
            .await
            .unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("vault"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert_eq!(universe.implicit_link_pairs().len(), 1);
        assert!(universe.implicit_link_pairs().contains(&(a, b)));
    });
}

#[test]
fn implicit_links_dangling_endpoints_are_skipped_on_restore() {
    with_rt(async {
        let (guard, storage) = temp_storage();
        let mut universe = Universe::open(storage).await.unwrap();
        let a = universe
            .create_note("Solo A".to_string(), "alpha\n".to_string())
            .await
            .unwrap();
        let ghost = penumbra_core::note::NoteId::new();
        let pairs = [(a, ghost), (ghost, a)].into_iter().collect();
        universe
            .storage()
            .save_implicit_links(&pairs)
            .await
            .unwrap();
        drop(universe);

        let reopen = pollster::block_on(Storage::with_dir(DirectoryHandle::from(
            guard.path().join("vault"),
        )));
        let universe = Universe::open(reopen).await.unwrap();
        assert!(universe.implicit_link_pairs().is_empty());
    });
}
