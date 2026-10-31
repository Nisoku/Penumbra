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

    assert!(
        dist > 0.0,
        "collision avoidance should separate overlapping nodes"
    );
    // A single step caps separation at max_collision_push per pass times the
    // pass count, so one step from fully coincident will not reach the full
    // target gap; assert only that the nodes moved meaningfully apart.
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
    // Same position; strip b's default box so no pair can be formed.
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));

    // Bounds only on a; b has none, so no overlapping pair is checked.
    engine.set_node_bounds(a, Bounds::new(100.0, 60.0));
    engine.clear_node_bounds(&b);

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
fn add_node_assigns_default_bounds() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let a = NoteId::new();
    engine.add_node(a, false);
    let bounds = engine
        .get_bounds(&a)
        .expect("add_node should assign a default box");
    assert_eq!(bounds, Bounds::new(180.0, 120.0));
}

#[test]
fn default_bounds_separate_overlapping_added_nodes() {
    let a = NoteId::new();
    let b = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // No explicit bounds call: the defaults from add_node must drive collision.
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));

    engine.step();

    let dist = engine
        .get_position(&a)
        .unwrap()
        .distance_to(&engine.get_position(&b).unwrap());
    assert!(
        dist > 10.0,
        "default bounds should separate coincident nodes, got {dist}"
    );
}

#[test]
fn settle_separates_unpinned_card_from_pinned_neighbor() {
    let pinned = NoteId::new();
    let other = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());

    engine.add_node(pinned, true);
    engine.add_node(other, false);
    engine.update_links(Vec::new());
    engine.set_position(&pinned, Position::new(7.875, 49.244));
    engine.set_position(&other, Position::new(7.942, 49.365));
    // Re-pin at the loaded position so the pin anchor matches the saved spot.
    engine.pin(&pinned, true);

    let pin_before = engine.get_position(&pinned).unwrap();

    for _ in 0..300 {
        engine.step();
    }

    let pin_after = engine.get_position(&pinned).unwrap();
    let other_after = engine.get_position(&other).unwrap();
    let bounds = Bounds::new(180.0, 120.0);

    assert!(
        pin_before.distance_to(&pin_after) < 1e-3,
        "pinned card moved: {pin_before:?} -> {pin_after:?}"
    );
    assert!(
        !bounds.overlaps(&pin_after, &bounds, &other_after),
        "unpinned card still overlaps its pinned neighbor: \
         pinned={pin_after:?} other={other_after:?}"
    );
}

#[test]
fn single_step_collision_separates_stacked_unlinked_nodes() {
    let a = NoteId::new();
    let b = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    engine.update_links(Vec::new());
    engine.set_position(&a, Position::new(7.875, 49.244));
    engine.set_position(&b, Position::new(7.942, 49.365));

    let before = engine
        .get_position(&a)
        .unwrap()
        .distance_to(&engine.get_position(&b).unwrap());
    engine.step();
    let after = engine
        .get_position(&a)
        .unwrap()
        .distance_to(&engine.get_position(&b).unwrap());
    assert!(
        after > before + 1.0,
        "one step moved the stacked cards only {before}->{after}"
    );
}

#[test]
fn boot_run_separates_stacked_unlinked_cards() {
    let a = NoteId::new();
    let b = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // Saved positions from a persisted stack.
    engine.set_position(&a, Position::new(7.875, 49.244));
    engine.set_position(&b, Position::new(7.942, 49.365));
    // No links, as the real vault has none between these two notes.
    engine.update_links(Vec::new());
    engine.run();

    let pa = engine.get_position(&a).unwrap();
    let pb = engine.get_position(&b).unwrap();
    let bounds = Bounds::new(180.0, 120.0);
    assert!(
        !bounds.overlaps(&pa, &bounds, &pb),
        "boot run left cards overlapping: a={pa:?} b={pb:?}"
    );
}

#[test]
fn settle_fully_separates_near_coincident_cards() {
    let a = NoteId::new();
    let b = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // Mimic a persisted overlap: two card-sized boxes ~0.14 units apart.
    engine.set_position(&a, Position::new(7.875, 49.244));
    engine.set_position(&b, Position::new(7.942, 49.365));

    // Run a generous number of settle frames, as the ambient loop does.
    for _ in 0..300 {
        engine.step();
    }

    let pa = engine.get_position(&a).unwrap();
    let pb = engine.get_position(&b).unwrap();
    let bounds = Bounds::new(180.0, 120.0);
    assert!(
        !bounds.overlaps(&pa, &bounds, &pb),
        "cards still overlap after settle: a={pa:?} b={pb:?}"
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

#[test]
fn cpu_fallback_moves_nodes() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let a = NoteId::new();
    let b = NoteId::new();
    let c = NoteId::new();
    engine.add_node(a, false);
    engine.add_node(b, false);
    engine.add_node(c, false);
    engine.set_position(&a, Position::new(0.0, 0.0));
    engine.set_position(&b, Position::new(200.0, 0.0));
    engine.set_position(&c, Position::new(100.0, 200.0));
    engine.update_links(vec![
        Link::new(a, b, LinkKind::Explicit),
        Link::new(b, c, LinkKind::Explicit),
    ]);
    let pos_a_before = engine.get_position(&a).unwrap();
    let pos_b_before = engine.get_position(&b).unwrap();
    let pos_c_before = engine.get_position(&c).unwrap();
    let d = engine.step();
    assert!(d.is_finite(), "displacement should be finite: {}", d);
    assert!(d > 0.0, "CPU fallback should move nodes");
    assert_ne!(
        engine.get_position(&a).unwrap(),
        pos_a_before,
        "node a should move"
    );
    assert_ne!(
        engine.get_position(&b).unwrap(),
        pos_b_before,
        "node b should move"
    );
    assert_ne!(
        engine.get_position(&c).unwrap(),
        pos_c_before,
        "node c should move"
    );
}

#[test]
fn cpu_fallback_converges() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let a = NoteId::new();
    let b = NoteId::new();
    let c = NoteId::new();
    let d = NoteId::new();
    engine.add_node(a, false);
    engine.add_node(b, false);
    engine.add_node(c, false);
    engine.add_node(d, false);
    engine.set_position(&a, Position::new(0.0, 0.0));
    engine.set_position(&b, Position::new(300.0, 0.0));
    engine.set_position(&c, Position::new(0.0, 300.0));
    engine.set_position(&d, Position::new(300.0, 300.0));
    engine.update_links(vec![
        Link::new(a, b, LinkKind::Explicit),
        Link::new(b, c, LinkKind::Explicit),
        Link::new(c, d, LinkKind::Explicit),
        Link::new(d, a, LinkKind::Explicit),
    ]);
    let mut prev_disp = f64::MAX;
    let mut decreased = false;
    for _ in 0..50 {
        let d = engine.step();
        if d < prev_disp {
            decreased = true;
        }
        prev_disp = d;
    }
    assert!(decreased, "displacement should decrease over iterations");
    assert!(
        prev_disp < 10.0,
        "layout should converge, final disp: {}",
        prev_disp
    );
}

#[test]
fn step_neighborhood_moves_local_nodes_only() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    // Two widely separated clusters; a neighborhood step on the left cluster
    // must not move the far right node.
    let left = NoteId::new();
    let near_left = NoteId::new();
    let far = NoteId::new();

    engine.add_node(left, false);
    engine.add_node(near_left, false);
    engine.add_node(far, false);

    // Default neighborhood radius is 400; the far node sits 5000 away so it
    // is well outside any local scope.
    engine.set_position(&left, Position::new(0.0, 0.0));
    engine.set_position(&near_left, Position::new(30.0, 0.0));
    engine.set_position(&far, Position::new(5000.0, 5000.0));

    engine.update_links(vec![Link::new(left, near_left, LinkKind::Explicit)]);

    let far_before = engine.get_position(&far).unwrap();
    let left_before = engine.get_position(&left).unwrap();

    let d = engine.step_neighborhood(&left);
    assert!(d.is_finite(), "displacement should be finite: {d}");

    let far_after = engine.get_position(&far).unwrap();
    let left_after = engine.get_position(&left).unwrap();

    // The far node is an anchor outside the neighborhood and must not move.
    assert_eq!(
        far_after, far_before,
        "distant node must not move during a neighborhood step"
    );
    // The center node should have been integrated.
    assert_ne!(
        left_after, left_before,
        "center node should move during a neighborhood step"
    );
}

#[test]
fn step_neighborhood_unknown_id_is_safe() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(NoteId::new(), false);
    let d = engine.step_neighborhood(&NoteId::new());
    assert_eq!(d, 0.0, "unknown node id yields zero displacement");
}

#[test]
fn step_neighborhood_resolves_local_collision() {
    let a = NoteId::new();
    let b = NoteId::new();
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    engine.add_node(a, false);
    engine.add_node(b, false);
    // Coincident so they overlap; both within the neighborhood of a.
    engine.set_position(&a, Position::new(100.0, 100.0));
    engine.set_position(&b, Position::new(100.0, 100.0));
    engine.set_node_bounds(a, Bounds::new(100.0, 60.0));
    engine.set_node_bounds(b, Bounds::new(100.0, 60.0));

    let _d = engine.step_neighborhood(&a);

    let pa = engine.get_position(&a).unwrap();
    let pb = engine.get_position(&b).unwrap();
    let dist = pa.distance_to(&pb);
    assert!(
        dist > 10.0,
        "nodes only {dist} apart after neighborhood resolve"
    );
}

#[test]
fn step_neighborhood_leaves_distant_cluster_untouched() {
    let mut engine = pollster::block_on(LayoutEngine::with_defaults());
    let mut ids = Vec::new();
    // A dense left cluster.
    for i in 0..20 {
        let id = NoteId::new();
        engine.add_node(id, false);
        engine.set_position(&id, Position::new(i as f64 * 10.0, 0.0));
        ids.push(id);
    }
    // A far away cluster.
    let mut far_ids = Vec::new();
    for i in 0..5 {
        let id = NoteId::new();
        engine.add_node(id, false);
        engine.set_position(&id, Position::new(i as f64 * 10.0 + 1.0e6, 1.0e6));
        far_ids.push(id);
    }
    let far_before: Vec<Position> = far_ids
        .iter()
        .map(|id| engine.get_position(id).unwrap())
        .collect();

    let _d = engine.step_neighborhood(&ids[0]);

    for (id, before) in far_ids.iter().zip(far_before.iter()) {
        assert_eq!(
            engine.get_position(id).unwrap(),
            *before,
            "far cluster nodes must be anchors"
        );
    }
}
