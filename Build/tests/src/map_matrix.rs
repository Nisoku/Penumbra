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
fn frame_card_centers_card_horizontally_at_reading_zoom() {
    let cam = map::frame_card(1000.0, 600.0, 800.0, 600.0);
    assert_eq!(cam.zoom, map::CARD_READING_ZOOM);
    let left = cam.x;
    let right = cam.x + 800.0 / cam.zoom;
    assert!(((left + right) * 0.5 - 1000.0).abs() < 0.001);
    let top = cam.y;
    let bottom = cam.y + 600.0 / cam.zoom;
    let center = (top + bottom) * 0.5;
    assert!(((center - 600.0).abs() - 0.12 * 600.0 / cam.zoom).abs() < 0.001);
}

#[test]
fn frame_card_scales_offsets_with_viewport_zoom() {
    let cam = map::frame_card(0.0, 0.0, 400.0, 300.0);
    assert_eq!(cam.x, -400.0 / map::CARD_READING_ZOOM * 0.5);
    assert_eq!(cam.y, -300.0 / map::CARD_READING_ZOOM * 0.38);
}

#[test]
fn zoom_card_grows_card_to_viewport_width() {
    let cam = map::zoom_card(1000.0, 600.0, 800.0, 600.0);
    assert!((cam.zoom * 180.0 - 800.0).abs() < 0.01);
}

#[test]
fn zoom_card_centers_card_horizontally_and_vertically() {
    let cam = map::zoom_card(1000.0, 600.0, 800.0, 600.0);
    assert_eq!(cam.x, 1000.0 - 800.0 / cam.zoom * 0.5);
    assert_eq!(cam.y, 600.0 - 600.0 / cam.zoom * 0.5);
}

#[test]
fn zoom_card_clamps_to_max_and_min_zoom() {
    let wide = map::zoom_card(0.0, 0.0, 4000.0, 2000.0);
    assert!(wide.zoom <= 8.0);
    let narrow = map::zoom_card(0.0, 0.0, 240.0, 200.0);
    assert!(narrow.zoom >= map::CARD_READING_ZOOM);
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
