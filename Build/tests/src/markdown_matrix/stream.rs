//! Incremental streaming parser coverage.

#[test]
fn stream_empty_initial() {
    let stream = penumbra_markdown::stream::MarkdownStream::new();
    assert!(stream.snapshot().is_empty());
}

#[test]
fn stream_append_para() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    let update = stream.append("hello world\n\n").unwrap();
    assert!(!update.committed.is_empty() || update.pending.is_some());
}

#[test]
fn stream_finalize_flushes() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("incomplete").unwrap();
    stream.finalize().unwrap();
    assert!(!stream.snapshot().is_empty());
}

#[test]
fn stream_reset_clears() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("hello\n\n").unwrap();
    stream.reset();
    assert!(stream.snapshot().is_empty());
}

#[test]
fn stream_snapshot_accumulates() {
    let mut stream = penumbra_markdown::stream::MarkdownStream::new();
    stream.append("# A\n\n").unwrap();
    stream.append("# B\n\n").unwrap();
    let snap1 = stream.snapshot();
    stream.append("# C\n\n").unwrap();
    let snap2 = stream.snapshot();
    assert!(snap2.blocks.len() >= snap1.blocks.len());
}
