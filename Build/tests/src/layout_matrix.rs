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
fn incremental_step_matches_full_step_within_tolerance() {
    let (mut engine_full, ids) = simple_graph();
    let mut engine_inc = LayoutEngine::with_defaults();

    for id in &ids {
        if let Some(pos) = engine_full.get_position(id) {
            engine_inc.add_node(*id, false);
            engine_inc.set_position(id, pos);
        }
    }
    engine_inc.update_links(vec![
        Link::new(ids[0], ids[1], LinkKind::Explicit),
        Link::new(ids[1], ids[2], LinkKind::Implicit),
    ]);

    // Pin the pinned node
    engine_inc.pin(&ids[2], true);

    let center = ids[2];
    engine_full.step();
    engine_inc.step_neighborhood(&center);

    for id in &ids {
        let fp = engine_full.get_position(id).unwrap();
        let ip = engine_inc.get_position(id).unwrap();
        let diff = fp.distance_to(&ip);
        assert!(diff < 1.0, "position diff too large: {}", diff);
    }
}

#[test]
fn empty_engine_does_not_panic() {
    let mut engine = LayoutEngine::with_defaults();
    let d = engine.step();
    assert_eq!(d, 0.0);
    assert_eq!(engine.iteration_count(), 1);
}
