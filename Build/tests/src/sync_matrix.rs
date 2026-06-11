use std::collections::HashMap;

use penumbra_core::note::Note;
use penumbra_sync::{GitSyncProvider, GoogleDriveSyncProvider, MockSyncProvider, SyncProvider};

fn make_note(title: &str, body: &str) -> Note {
    Note::new(title.to_string(), body.to_string())
}

// MockSyncProvider tests
#[test]
fn mock_push_and_pull_roundtrip() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let provider = MockSyncProvider::new();

        let notes = vec![make_note("hello", "world")];
        let embeddings = HashMap::new();
        let positions = HashMap::new();

        let snapshot = provider
            .push(&notes, &embeddings, &positions, None)
            .await
            .expect("push should succeed");

        assert!(snapshot.note_count > 0);

        let result = provider.pull(None).await.expect("pull should succeed");
        assert_eq!(result.notes.len(), 1);
        assert!(result.notes.contains_key(&notes[0].id));
    });
}

#[test]
fn mock_status_returns_note_count() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let provider = MockSyncProvider::new();

        let notes = vec![make_note("a", "aa"), make_note("b", "bb")];
        provider
            .push(&notes, &HashMap::new(), &HashMap::new(), None)
            .await
            .unwrap();

        let status = provider.status().await.expect("status should succeed");
        assert_eq!(status.note_count, 2);
        assert!(status.snapshot_id.is_some());
    });
}

#[test]
fn mock_fail_mode_returns_errors() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let provider = MockSyncProvider::new();
        provider.set_fail(true);

        let result = provider
            .push(&[], &HashMap::new(), &HashMap::new(), None)
            .await;
        assert!(result.is_err(), "push should fail in fail mode");

        let result = provider.pull(None).await;
        assert!(result.is_err(), "pull should fail in fail mode");

        let result = provider.status().await;
        assert!(result.is_err(), "status should fail in fail mode");
    });
}

#[test]
fn mock_last_sync_updated_after_push() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let provider = MockSyncProvider::new();
        assert!(provider.last_sync().is_none());

        provider
            .push(&[], &HashMap::new(), &HashMap::new(), None)
            .await
            .unwrap();

        assert!(provider.last_sync().is_some());
    });
}

// Stub provider tests
#[test]
fn git_sync_provider_connect_returns_not_implemented() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let mut provider = GitSyncProvider::new("/tmp/repo", "https://example.com/repo");
        let result = provider.connect().await;
        assert!(result.is_err(), "git sync stub should return error");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented"),
            "error should mention stub status"
        );
    });
}

#[test]
fn gdrive_sync_provider_connect_returns_not_implemented() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let mut provider = GoogleDriveSyncProvider::new("test-client-id");
        let result = provider.connect().await;
        assert!(result.is_err(), "gdrive sync stub should return error");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented"),
            "error should mention stub status"
        );
    });
}
