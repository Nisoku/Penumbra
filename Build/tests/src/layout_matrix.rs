use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::NoteId;
use penumbra_layout::LayoutEngine;

fn simple_graph() -> (LayoutEngine, Vec<NoteId>) {
    let mut engine = LayoutEngine::with_defaults();
    let a = NoteId::new();
    let b = NoteId::new();
    let c = NoteId::new();
    engine.add_node(a, false);
    engine.add_node(b, false);
    engine.add_node(c, true);
    engine.update_links(vec![
        Link::new(a, b, LinkKind::Explicit),
        Link::new(b, c, LinkKind::Implicit),
    ]);
    (engine, vec![a, b, c])
}

#[test]
fn layout_runs_steps() {
    let (mut engine, _ids) = simple_graph();
    for _ in 0..10 {
        let d = engine.step();
        assert!(d.is_finite());
    }
    assert!(engine.iteration_count() > 0);
}

#[test]
fn pinned_node_does_not_move() {
    let mut engine = LayoutEngine::with_defaults();
    let pinned = NoteId::new();
    let other = NoteId::new();
    engine.add_node(pinned, true);
    engine.add_node(other, false);
    let pos_before = engine.get_position(&pinned).unwrap();
    engine.run();
    let pos_after = engine.get_position(&pinned).unwrap();
    assert_eq!(pos_before, pos_after);
}

#[test]
fn all_positions_are_finite() {
    let (mut engine, ids) = simple_graph();
    engine.run();
    for id in &ids {
        let pos = engine.get_position(id).unwrap();
        assert!(pos.x.is_finite());
        assert!(pos.y.is_finite());
    }
}

#[test]
fn converges_eventually() {
    let (mut engine, _ids) = simple_graph();
    let iterations = engine.run();
    assert!(iterations > 0);
    assert!(engine.iteration_count() <= 200);
}

#[test]
fn step_neighborhood_moves_only_affected_nodes() {
    let (mut engine, ids) = simple_graph();
    // ids[2] is pinned. Its neighbor is ids[1] via the implicit link.
    // step_neighborhood should move ids[0] (not affected), move ids[1]
    // (affected, not pinned), and not move ids[2] (affected, pinned).
    let center = ids[2];
    let pos_a_before = engine.get_position(&ids[0]).unwrap();
    let pos_b_before = engine.get_position(&ids[1]).unwrap();
    let pos_c_before = engine.get_position(&ids[2]).unwrap();

    let d = engine.step_neighborhood(&center);

    assert!(d.is_finite(), "displacement should be finite: {}", d);
    assert!(d > 0.0, "should have moved at least one node");
    // Non-neighbor should NOT move
    assert_eq!(
        engine.get_position(&ids[0]).unwrap(),
        pos_a_before,
        "non-neighbor node should not move"
    );
    // Pinned neighbor should NOT move
    assert_eq!(
        engine.get_position(&ids[2]).unwrap(),
        pos_c_before,
        "pinned node should not move"
    );
    // Unpinned neighbor SHOULD move
    assert_ne!(
        engine.get_position(&ids[1]).unwrap(),
        pos_b_before,
        "unpinned neighbor should move"
    );
}

#[test]
fn empty_engine_does_not_panic() {
    let mut engine = LayoutEngine::with_defaults();
    let d = engine.step();
    assert_eq!(d, 0.0);
    assert_eq!(engine.iteration_count(), 1);
}
