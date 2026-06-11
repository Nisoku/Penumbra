use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::NoteId;
use penumbra_core::position::{Bounds, Position};
use penumbra_layout::LayoutEngine;

fn simple_graph() -> (LayoutEngine, Vec<NoteId>) {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let a = NoteId::new();
    let b = NoteId::new();
    let c = NoteId::new();
    engine.add_node(a, false);
    engine.add_node(b, false);
    engine.add_node(c, true);
    let links = vec![
        Link::new(a, b, LinkKind::Explicit),
        Link::new(b, c, LinkKind::Implicit),
    ];
    engine.update_links(links);
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
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
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
fn step_moves_unpinned_nodes() {
    let (mut engine, ids) = simple_graph();
    // ids[2] is pinned, ids[0] and ids[1] are unpinned.
    let pos_a_before = engine.get_position(&ids[0]).unwrap();
    let pos_b_before = engine.get_position(&ids[1]).unwrap();
    let pos_c_before = engine.get_position(&ids[2]).unwrap();

    let d = engine.step();

    assert!(d.is_finite(), "displacement should be finite: {}", d);
    assert!(d > 0.0, "should have moved at least one node");
    // Unpinned nodes SHOULD move
    assert_ne!(
        engine.get_position(&ids[0]).unwrap(),
        pos_a_before,
        "unpinned node should move"
    );
    assert_ne!(
        engine.get_position(&ids[1]).unwrap(),
        pos_b_before,
        "unpinned node should move"
    );
    // Pinned node should NOT move
    assert_eq!(
        engine.get_position(&ids[2]).unwrap(),
        pos_c_before,
        "pinned node should not move"
    );
}

#[test]
fn empty_engine_does_not_panic() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let d = engine.step();
    assert_eq!(d, 0.0);
    assert_eq!(engine.iteration_count(), 1);
}

#[test]
fn collision_resolves_overlap() {
    let a = NoteId::new();
    let b = NoteId::new();

    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // Place nodes at the same position so they overlap.
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));

    let bounds = Bounds::new(100.0, 60.0);
    engine.set_node_bounds(a, bounds);
    engine.set_node_bounds(b, bounds);

    // Step triggers collision resolution.
    let _d = engine.step();

    let pa = engine.get_position(&a).unwrap();
    let pb = engine.get_position(&b).unwrap();
    let dist = pa.distance_to(&pb);

    // Nodes should have been pushed apart.
    assert!(
        dist > 0.0,
        "collision avoidance should separate overlapping nodes"
    );
    // Minimum separation: half-diagonal + half-diagonal + margin.
    // half_diag = sqrt(100^2 + 60^2) / 2 = 58.31, margin = 10
    // min_dist = 58.31 + 58.31 + 10.0 = 126.62, but with only 5 passes
    // and max_push = 20 per pass, we may not reach full separation.
    // Just verify they moved apart.
    assert!(
        dist > 10.0,
        "nodes only {dist} apart after collision resolve"
    );
}

#[test]
fn collision_skipped_without_bounds() {
    let a = NoteId::new();
    let b = NoteId::new();

    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // Same position, but NO bounds set. Collision avoidance is a no-op.
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));

    // Bounds only set for a, not b. Should not crash or move.
    engine.set_node_bounds(a, Bounds::new(100.0, 60.0));

    let _d = engine.step();
    // b has no bounds so no pair is checked against it.
    let pa = engine.get_position(&a).unwrap();
    let pb = engine.get_position(&b).unwrap();
    assert!(
        (pa.x - pb.x).abs() < 0.01 && (pa.y - pb.y).abs() < 0.01,
        "nodes should not have moved without matching bounds"
    );
}

#[test]
fn collision_pinned_node_stays() {
    let a = NoteId::new();
    let b = NoteId::new();

    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, true); // pinned
    engine.add_node(b, false);
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));

    let bounds = Bounds::new(100.0, 60.0);
    engine.set_node_bounds(a, bounds);
    engine.set_node_bounds(b, bounds);

    let pos_a_before = engine.get_position(&a).unwrap();
    let _d = engine.step();
    let pos_a_after = engine.get_position(&a).unwrap();
    assert_eq!(
        pos_a_before, pos_a_after,
        "pinned node should not move during collision resolution"
    );
}
