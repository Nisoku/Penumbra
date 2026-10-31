use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::map::MapCamera;
use crate::rich_editor::RichEditor;
use crate::platform::Instant;
use crate::NoteCardVM;
use futures::lock::Mutex as AsyncMutex;
use penumbra_app::Universe;
use penumbra_auto_link::AutoLinker;
use penumbra_core::note::NoteId;
use penumbra_core::position::Position;
use penumbra_core::EmbeddingProvider;
use penumbra_layout::LayoutEngine;
use slint::Model;

thread_local! {
    pub(crate) static UI_CARDS_MODEL: std::cell::RefCell<Option<Rc<slint::VecModel<NoteCardVM>>>> = const {
        std::cell::RefCell::new(None)
    };
}

pub(crate) const WELCOME_TITLE: &str = "Welcome to Penumbra";
pub(crate) const WELCOME_BODY: &str =
    "This is your first note.\n\nEverything you write lands on the map over time. Drag notes around, link them, and watch constellations form.";

pub(crate) const TOP_BAR_HEIGHT: f32 = 52.0;

pub(crate) const DRIFT_DURATION: Duration = Duration::from_millis(600);
pub(crate) const DRIFT_TICK: Duration = Duration::from_millis(16);
pub(crate) const GLIDE_DURATION: Duration = Duration::from_millis(450);
pub(crate) const OPEN_ZOOM_DURATION: Duration = Duration::from_millis(520);
pub(crate) const GLIDE_TICK: Duration = Duration::from_millis(16);
pub(crate) const GLIDE_EPSILON: f32 = 0.5;
pub(crate) const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
pub(crate) const PERSIST_INTERVAL: Duration = Duration::from_secs(2);

/// Delay between ambient-settle frames.
pub(crate) const SETTLE_TICK: Duration = Duration::from_millis(40);
/// Global layout steps advanced per ambient-settle frame.
pub(crate) const SETTLE_STEPS_PER_FRAME: usize = 3;
/// Mean per-node displacement (world px) below which the map is considered
/// settled and the ambient loop stops.
pub(crate) const SETTLE_CONVERGENCE: f64 = 0.35;
/// Upper bound on ambient-settle frames so the loop always terminates even if
/// the simulation keeps jittering.
pub(crate) const SETTLE_MAX_FRAMES: u32 = 240;

/// Fraction of the viewport the fitted constellation fills, leaving a margin so
/// cards never touch the edges.
pub(crate) const FIT_MARGIN: f32 = 0.88;
/// Smallest fit-to-view zoom, so a very spread constellation stays legible
/// rather than collapsing to specks.
pub(crate) const FIT_MIN_ZOOM: f32 = 0.35;
/// Largest fit-to-view zoom, so a single note is framed comfortably instead of
/// filling the whole plane.
pub(crate) const FIT_MAX_ZOOM: f32 = 1.4;

/// The single state object shared across every event handler
pub(crate) struct SharedState {
    pub(crate) universe: AsyncMutex<Option<Universe>>,
    pub(crate) layout: AsyncMutex<Option<LayoutEngine>>,
    pub(crate) cards: Mutex<Vec<NoteCardVM>>,
    pub(crate) links: Mutex<Vec<(i32, i32)>>,
    pub(crate) pins: Mutex<HashMap<i32, (f32, f32)>>,
    pub(crate) selected: AtomicI32,
    pub(crate) dirty: AtomicBool,
    pub(crate) drift: Mutex<Option<Drift>>,
    pub(crate) editor: Mutex<Option<RichEditor>>,
    pub(crate) editor_note: Mutex<Option<NoteId>>,
    pub(crate) editor_title: Mutex<Option<String>>,
    pub(crate) editor_dirty: AtomicBool,
    pub(crate) edit_pending: AtomicI32,
    pub(crate) glide: Mutex<Option<CameraGlide>>,
    pub(crate) editor_prev_camera: Mutex<Option<MapCamera>>,
    pub(crate) editor_reveal: AtomicBool,
    pub(crate) embedder: Mutex<Option<Arc<dyn EmbeddingProvider>>>,
    pub(crate) linker: Mutex<Option<Arc<AutoLinker>>>,
    /// True while the ambient-settle loop is running, so only one is ever
    /// armed at a time; a fresh request just extends the running loop.
    pub(crate) settling: AtomicBool,
    /// The in-progress manual link drag, if the user is dragging from a card.
    pub(crate) pending_link: Mutex<Option<PendingLink>>,
    /// Set once the user pans or zooms the map by hand.
    pub(crate) user_panned: AtomicBool,
}

/// An in-progress manual link drag from a source card to the pointer.
pub(crate) struct PendingLink {
    pub(crate) source_card: i32,
    /// Current pointer position in world coordinates.
    pub(crate) pointer: (f32, f32),
}

/// One card's in-flight settle animation: interpolate `from` towards `to`.
pub(crate) struct DriftTarget {
    pub(crate) card_id: i32,
    pub(crate) from: (f32, f32),
    pub(crate) to: (f32, f32),
}

/// A batch of card drifts animating together over `duration`.
pub(crate) struct Drift {
    pub(crate) items: Vec<DriftTarget>,
    pub(crate) started: Instant,
    pub(crate) duration: Duration,
}

/// An in-flight camera animation between two viewpoints.
pub(crate) struct CameraGlide {
    pub(crate) from: MapCamera,
    pub(crate) to: MapCamera,
    pub(crate) started: Instant,
    pub(crate) duration: Duration,
}

/// Set the window status line, tolerating a dropped window handle.
pub(crate) fn set_status(ui: &slint::Weak<crate::AppWindow>, msg: &str) {
    if let Some(ui) = ui.upgrade() {
        ui.set_status(msg.into());
    }
}

/// 32-bit card identifier derived from a note's UUID.
pub(crate) fn note_code(id: &NoteId) -> i32 {
    id.as_uuid().as_u128() as i32
}

/// Build the UI card view-models from the current layout and graph, marking
/// each card pinned if its note id is in `pinned`.
pub(crate) fn build_note_cards(
    layout: &LayoutEngine,
    graph: &penumbra_graph::GraphStore,
    pinned: &HashSet<NoteId>,
) -> Vec<NoteCardVM> {
    let positions = layout.all_positions();
    graph
        .all_notes()
        .map(|note| {
            let pos = positions
                .get(&note.id)
                .copied()
                .unwrap_or(Position::new(0.0, 0.0));
            NoteCardVM {
                id: note_code(&note.id),
                title: note.title.as_str().into(),
                preview: note
                    .body
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .as_str()
                    .into(),
                tags: note.tags.join(", ").as_str().into(),
                pinned: pinned.contains(&note.id),
                x: pos.x as f32,
                y: pos.y as f32,
            }
        })
        .collect()
}

/// Push one updated card row into the live Slint model by index
pub(crate) fn apply_card_visual_model(idx: usize, row: NoteCardVM) {
    UI_CARDS_MODEL.with(|cell| {
        if let Some(model) = cell.borrow().as_ref() {
            model.set_row_data(idx, row);
        }
    });
}

/// Translate the UI's pinned card codes back into the note ids they stand for.
pub(crate) fn pinned_note_ids(universe: &Universe, pin_codes: &HashSet<i32>) -> HashSet<NoteId> {
    universe
        .graph()
        .all_notes()
        .map(|note| note.id)
        .filter(|id| pin_codes.contains(&note_code(id)))
        .collect()
}

pub(crate) fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
