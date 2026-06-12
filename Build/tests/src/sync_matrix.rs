use std::collections::HashMap;
use std::future::Future;

use penumbra_core::note::Note;
use penumbra_sync::{
    GitSyncProvider, GoogleDriveSyncProvider, MockSyncProvider, SyncProvider, WorkerSyncProvider,
};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_note(title: &str, body: &str) -> Note {
    Note::new(title.to_string(), body.to_string())
}

fn status_body() -> serde_json::Value {
    serde_json::json!({
        "noteCount": 3,
        "lastModified": "2026-06-11T12:00:00Z",
        "snapshotId": "snap-abc",
        "storageBytes": 4096,
        "storageLimit": 536870912
    })
}

fn with_rt(f: impl Future<Output = ()>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();
    rt.block_on(f);
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

// WorkerSyncProvider (wiremock) tests

#[test]
fn worker_connect_sends_get_and_returns_ok() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_body()))
            .mount(&mock)
            .await;

        let mut provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider.connect().await;
        assert!(result.is_ok());
    });
}

#[test]
fn worker_connect_non_200_returns_error() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/status"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock)
            .await;

        let mut provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider.connect().await;
        assert!(result.is_err());
    });
}

#[test]
fn worker_push_returns_snapshot() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/push"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 2,
                "snapshotId": "snap-1"
            })))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let notes = vec![make_note("a", "aa"), make_note("b", "bb")];
        let snapshot = provider
            .push(&notes, &HashMap::new(), &HashMap::new(), None)
            .await
            .expect("push should succeed");

        assert_eq!(snapshot.snapshot_id, "snap-1");
        assert_eq!(snapshot.note_count, 2);
    });
}

#[test]
fn worker_push_forwards_snapshot_id() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/push"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 1,
                "snapshotId": "snap-2"
            })))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let notes = vec![make_note("x", "y")];
        let snapshot = provider
            .push(&notes, &HashMap::new(), &HashMap::new(), Some("prev-snap"))
            .await
            .expect("push should succeed");

        assert_eq!(snapshot.snapshot_id, "snap-2");
    });
}

#[test]
fn worker_push_500_returns_error() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/push"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider
            .push(&[], &HashMap::new(), &HashMap::new(), None)
            .await;

        assert!(result.is_err());
    });
}

#[test]
fn worker_push_409_returns_conflict() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/push"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "error": "conflict",
                "currentSnapshotId": "server-snap-42"
            })))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider
            .push(&[], &HashMap::new(), &HashMap::new(), Some("stale-snap"))
            .await;

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("409") || msg.contains("conflict"));
    });
}

#[test]
fn worker_pull_returns_notes_embeddings_positions() {
    with_rt(async {
        let mock = MockServer::start().await;
        let note = make_note("hello", "world");
        let body = serde_json::json!({
            "notes": { note.id.to_string(): serde_json::to_value(&note).unwrap() },
            "embeddings": {},
            "positions": {},
            "snapshot": {
                "snapshotId": "snap-3",
                "timestamp": "2026-06-11T12:00:00Z",
                "noteCount": 1,
                "noteIds": []
            }
        });

        Mock::given(method("POST"))
            .and(path("/sync/pull"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider.pull(None).await.expect("pull should succeed");

        assert_eq!(result.notes.len(), 1);
        assert!(result.notes.contains_key(&note.id));
        assert!(result.snapshot.is_some());
    });
}

#[test]
fn worker_pull_sends_since_query_param() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/pull"))
            .and(query_param("since", "snap-prev"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "notes": {},
                "embeddings": {},
                "positions": {}
            })))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let result = provider.pull(Some("snap-prev")).await;
        assert!(result.is_ok());
    });
}

#[test]
fn worker_status_returns_formatted_result() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_body()))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        let status = provider.status().await.expect("status should succeed");

        assert_eq!(status.note_count, 3);
        assert_eq!(status.storage_bytes, 4096);
        assert_eq!(status.snapshot_id.as_deref(), Some("snap-abc"));
        assert!(status.last_modified.is_some());
    });
}

#[test]
fn worker_last_sync_none_initially() {
    let provider = WorkerSyncProvider::new("http://localhost:9999");
    assert!(provider.last_sync().is_none());
}

#[test]
fn worker_last_sync_updated_after_push() {
    with_rt(async {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/push"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 0,
                "snapshotId": "s"
            })))
            .mount(&mock)
            .await;

        let provider = WorkerSyncProvider::new(&mock.uri());
        assert!(provider.last_sync().is_none());

        provider
            .push(&[], &HashMap::new(), &HashMap::new(), None)
            .await
            .unwrap();

        assert!(provider.last_sync().is_some());
    });
}
