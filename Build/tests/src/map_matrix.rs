use penumbra_ui::map::{self, MapCamera};
use penumbra_ui::NoteCardVM;

fn card(id: i32, x: f32, y: f32) -> NoteCardVM {
    NoteCardVM {
        id,
        title: format!("note {id}").into(),
        preview: String::new().into(),
        tags: String::new().into(),
        pinned: false,
        x,
        y,
    }
}

#[test]
fn edge_path_uses_screen_coordinates_and_culls_offscreen() {
    let cards = vec![card(1, 0.0, 0.0), card(2, 200.0, 0.0)];
    let links = vec![(1, 2)];
    let cam = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let edges = map::build_edges(&cards, &links, cam, 800.0, 600.0, -1);
    assert!(edges.base.contains("M 0.0 0.0 C"));
    assert!(edges.base.contains("200.0 0.0 "));
    assert!(edges.selected.is_empty());

    let far = vec![card(1, 5000.0, 5000.0), card(2, 5200.0, 5000.0)];
    let edges = map::build_edges(&far, &links, cam, 800.0, 600.0, -1);
    assert!(edges.base.is_empty());
}

#[test]
fn edge_path_highlights_only_selected_neighbourhood() {
    let cards = vec![card(1, 0.0, 0.0), card(2, 200.0, 0.0), card(3, 400.0, 0.0)];
    let links = vec![(1, 2), (2, 3), (1, 3)];
    let cam = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let edges = map::build_edges(&cards, &links, cam, 800.0, 600.0, 2);
    assert!(edges.selected.contains("200.0 0.0 "));
    assert!(edges.base.contains("400.0 0.0 "));
    assert!(!edges.base.contains("200.0 0.0 "));
    assert!(edges.selected.contains("0.0 0.0 "));
}

#[test]
fn edge_path_skips_links_without_cards() {
    let cards = vec![card(1, 0.0, 0.0), card(2, 120.0, 0.0)];
    let links = vec![(1, 2), (1, 99), (98, 99)];
    let cam = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let edges = map::build_edges(&cards, &links, cam, 800.0, 600.0, -1);
    assert!(edges.base.contains("M 0.0 0.0 C"));
    assert_eq!(edges.base.matches("M ").count(), 1);
}

#[test]
fn edge_curve_bend_is_clamped() {
    let cards = vec![card(1, 0.0, 0.0), card(2, 2.0, 0.0)];
    let links = vec![(1, 2)];
    let cam = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let edges = map::build_edges(&cards, &links, cam, 800.0, 600.0, -1);
    assert!(edges.base.contains("M 0.0 0.0 C 1.0"));
}

#[test]
fn grid_path_covers_viewport_at_spacing() {
    let cam = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let path = map::build_grid(cam, 600.0, 480.0, 120.0);
    let vertical = 600.0 / 120.0 + 1.0;
    let horizontal = 480.0 / 120.0 + 1.0;
    assert_eq!(
        path.matches("M ").count(),
        vertical as usize + horizontal as usize
    );
}

#[test]
fn grid_path_anchors_to_camera_and_clamps_density() {
    let cam = MapCamera {
        x: 5000.0,
        y: -3000.0,
        zoom: 1.0,
    };
    let path = map::build_grid(cam, 800.0, 600.0, 120.0);
    assert!(path.starts_with("M -80.0 0 L -80.0"));
    assert!(path.contains("M 0 0.0 L 800.0 0.0 "));

    let shrunk = MapCamera {
        x: 0.0,
        y: 0.0,
        zoom: 0.01,
    };
    let dense = map::build_grid(shrunk, 4000.0, 3000.0, 120.0);
    let lines = dense.matches("M ").count();
    assert!(lines < 400);
}
