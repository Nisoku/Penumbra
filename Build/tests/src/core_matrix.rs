use penumbra_core::error::PenumbraError;
use penumbra_core::link::{Link, LinkKind};
use penumbra_core::note::{Note, NoteId, NoteMeta};
use penumbra_core::position::{Bounds, Position};
use penumbra_core::tag::Tag;

// NoteId

#[test]
fn note_id_new_unique() {
    let a = NoteId::new();
    let b = NoteId::new();
    assert_ne!(a, b);
}

#[test]
fn note_id_from_raw_roundtrip() {
    let id = NoteId::new();
    let raw = *id.as_uuid();
    let restored = NoteId::from_raw(raw);
    assert_eq!(id, restored);
}

#[test]
fn note_id_copy_works() {
    let a = NoteId::new();
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn note_id_hash_works() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    let a = NoteId::new();
    let b = NoteId::new();
    set.insert(a);
    set.insert(b);
    assert_eq!(set.len(), 2);
    set.insert(a);
    assert_eq!(set.len(), 2);
}

#[test]
fn note_id_display_is_uuid() {
    let id = NoteId::new();
    let s = id.to_string();
    assert_eq!(s.len(), 36);
    assert_eq!(s.chars().filter(|&c| c == '-').count(), 4);
}

// Note

#[test]
fn note_new_sets_fields() {
    let note = Note::new("Title".into(), "Body".into());
    assert_eq!(note.title, "Title");
    assert_eq!(note.body, "Body");
    assert!(note.tags.is_empty());
    assert!(!note.meta.pinned);
    assert!(!note.meta.archived);
}

#[test]
fn note_touch_updates_timestamp() {
    let mut note = Note::new("T".into(), "B".into());
    let before = note.meta.updated_at;
    std::thread::sleep(std::time::Duration::from_millis(5));
    note.touch();
    assert!(note.meta.updated_at > before);
}

#[test]
fn note_meta_default_not_pinned() {
    let meta = NoteMeta::default();
    assert!(!meta.pinned);
    assert!(!meta.archived);
}

// Link

#[test]
fn link_new_explicit_weight() {
    let a = NoteId::new();
    let b = NoteId::new();
    let link = Link::new(a, b, LinkKind::Explicit);
    assert!((link.weight - 1.0).abs() < 1e-10);
    assert_eq!(link.kind, LinkKind::Explicit);
    assert_eq!(link.source, a);
    assert_eq!(link.target, b);
}

#[test]
fn link_new_implicit_weight() {
    let a = NoteId::new();
    let b = NoteId::new();
    let link = Link::new(a, b, LinkKind::Implicit);
    assert!((link.weight - 0.5).abs() < 1e-10);
}

#[test]
fn link_with_weight_override() {
    let a = NoteId::new();
    let b = NoteId::new();
    let link = Link::with_weight(a, b, LinkKind::Explicit, 3.0);
    assert!((link.weight - 3.0).abs() < 1e-10);
}

#[test]
fn link_other_endpoint() {
    let a = NoteId::new();
    let b = NoteId::new();
    let link = Link::new(a, b, LinkKind::Explicit);
    assert_eq!(*link.other(&a).unwrap(), b);
    assert_eq!(*link.other(&b).unwrap(), a);
}

#[test]
fn link_other_unrelated_id() {
    let a = NoteId::new();
    let b = NoteId::new();
    let c = NoteId::new();
    let link = Link::new(a, b, LinkKind::Explicit);
    assert!(link.other(&c).is_none());
}

// Position

#[test]
fn position_distance_3_4_5() {
    let p1 = Position::new(0.0, 0.0);
    let p2 = Position::new(3.0, 4.0);
    assert!((p1.distance_to(&p2) - 5.0).abs() < 1e-12);
    assert!((p1.squared_distance_to(&p2) - 25.0).abs() < 1e-12);
}

#[test]
fn position_zero_distance() {
    let p = Position::new(1.5, 2.5);
    assert_eq!(p.distance_to(&p), 0.0);
}

#[test]
fn position_addition() {
    let a = Position::new(1.0, 2.0);
    let b = Position::new(3.0, 4.0);
    let sum = a + b;
    assert!((sum.x - 4.0).abs() < 1e-12);
    assert!((sum.y - 6.0).abs() < 1e-12);
}

#[test]
fn position_subtraction() {
    let a = Position::new(5.0, 8.0);
    let b = Position::new(2.0, 3.0);
    let diff = a - b;
    assert!((diff.x - 3.0).abs() < 1e-12);
    assert!((diff.y - 5.0).abs() < 1e-12);
}

#[test]
fn position_scalar_mul() {
    let p = Position::new(2.0, 3.0);
    let scaled = p * 0.5;
    assert!((scaled.x - 1.0).abs() < 1e-12);
    assert!((scaled.y - 1.5).abs() < 1e-12);
}

#[test]
fn position_from_tuple() {
    let p: Position = (3.5, 7.2).into();
    assert!((p.x - 3.5).abs() < 1e-12);
    assert!((p.y - 7.2).abs() < 1e-12);
}

// Bounds

#[test]
fn bounds_contains_inside() {
    let bounds = Bounds::new(10.0, 10.0);
    let pos = Position::new(5.0, 5.0);
    let inside = Position::new(7.0, 7.0);
    assert!(bounds.contains(&pos, &inside));
}

#[test]
fn bounds_contains_outside() {
    let bounds = Bounds::new(10.0, 10.0);
    let pos = Position::new(0.0, 0.0);
    // Bounds extends from (0,0) to (10,10), so (11, 11) is outside
    let outside = Position::new(11.0, 11.0);
    assert!(!bounds.contains(&pos, &outside));
}

#[test]
fn bounds_overlaps_adjacent() {
    let b = Bounds::new(10.0, 10.0);
    let pos_a = Position::new(0.0, 0.0);
    // pos_b covers [9.0, 19.0), overlaps [0, 10) at [9, 10)
    let pos_b = Position::new(9.0, 0.0);
    assert!(b.overlaps(&pos_a, &b, &pos_b));
}

#[test]
fn bounds_overlaps_separate() {
    let b = Bounds::new(10.0, 10.0);
    let pos_a = Position::new(0.0, 0.0);
    // pos_b covers [15.0, 25.0), no overlap with [0, 10)
    let pos_b = Position::new(15.0, 0.0);
    assert!(!b.overlaps(&pos_a, &b, &pos_b));
}

#[test]
fn bounds_default_size() {
    let bounds = Bounds::default();
    assert!((bounds.width - 200.0).abs() < 1e-12);
    assert!((bounds.height - 150.0).abs() < 1e-12);
}

#[test]
fn bounds_zero() {
    let zero = Bounds::zero();
    assert_eq!(zero.width, 0.0);
    assert_eq!(zero.height, 0.0);
}

// Tag

#[test]
fn tag_new_no_color() {
    let tag = Tag::new("important".into());
    assert_eq!(tag.name, "important");
    assert!(tag.color.is_none());
}

#[test]
fn tag_with_color() {
    let tag = Tag::with_color("urgent".into(), "#ff0000".into());
    assert_eq!(tag.name, "urgent");
    assert_eq!(tag.color.as_deref(), Some("#ff0000"));
}

#[test]
fn tag_from_string() {
    let tag: Tag = "hello".to_string().into();
    assert_eq!(tag.name, "hello");
    assert!(tag.color.is_none());
}

#[test]
fn tag_display() {
    let tag = Tag::new("test-tag".into());
    assert_eq!(tag.to_string(), "test-tag");
}

// Serialization roundtrips

#[test]
fn serde_note_roundtrip() {
    let note = Note::new("Title".into(), "Body".into());
    let json = serde_json::to_string(&note).unwrap();
    let restored: Note = serde_json::from_str(&json).unwrap();
    assert_eq!(note.id, restored.id);
    assert_eq!(note.title, restored.title);
    assert_eq!(note.body, restored.body);
}

#[test]
fn serde_link_roundtrip() {
    let link = Link::new(NoteId::new(), NoteId::new(), LinkKind::Explicit);
    let json = serde_json::to_string(&link).unwrap();
    let restored: Link = serde_json::from_str(&json).unwrap();
    assert_eq!(link, restored);
}

#[test]
fn serde_position_roundtrip() {
    let pos = Position::new(1.5, 2.5);
    let json = serde_json::to_string(&pos).unwrap();
    let restored: Position = serde_json::from_str(&json).unwrap();
    assert!((restored.x - 1.5).abs() < 1e-12);
}

#[test]
fn serde_note_id_roundtrip() {
    let id = NoteId::new();
    let json = serde_json::to_string(&id).unwrap();
    let restored: NoteId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, restored);
}

// Errors

#[test]
fn error_serialization_from_serde() {
    let invalid = "not valid json at all";
    let result: Result<Note, _> = serde_json::from_str(invalid);
    let err = PenumbraError::from(result.unwrap_err());
    assert!(matches!(err, PenumbraError::Serialization(_)));
}
