use std::collections::HashMap;

use penumbra_core::link::Link;
use penumbra_core::note::{Note, NoteId};
use penumbra_core::position::Position;
use penumbra_events::{Event, EventBus};

#[test]
fn new_bus_empty() {
    let bus = EventBus::new();
    // No subscribers yet, publish should be a no-op
    bus.try_publish(Event::NoteAdded {
        id: NoteId::new(),
        note: Note::new("".into(), "".into()),
    });
}

#[test]
fn subscribe_receives_published_event() {
    let bus = EventBus::new();
    let rx = bus.subscribe();
    let id = NoteId::new();
    bus.try_publish(Event::NoteRemoved { id });
    let received = rx.try_recv().ok();
    assert!(received.is_some());
    match received.unwrap() {
        Event::NoteRemoved { id: rid } => assert_eq!(rid, id),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn all_event_variants_can_be_published() {
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let id = NoteId::new();
    let note = Note::new("test".into(), "body".into());
    let id2 = NoteId::new();
    let link = Link::new(id, id2, penumbra_core::link::LinkKind::Explicit);
    let mut positions = HashMap::new();
    positions.insert(id, Position::new(1.0, 2.0));

    let events = vec![
        Event::NoteAdded {
            id,
            note: note.clone(),
        },
        Event::NoteUpdated {
            id,
            note: note.clone(),
        },
        Event::NoteRemoved { id },
        Event::NotePinned { id },
        Event::NoteUnpinned { id },
        Event::LinkAdded { link: link.clone() },
        Event::LinkRemoved {
            source: id,
            target: id2,
        },
        Event::LayoutChanged {
            positions: positions.clone(),
        },
        Event::EmbeddingReady { id },
        Event::StateRestored {
            positions: positions.clone(),
        },
    ];

    for event in &events {
        bus.try_publish(event.clone());
    }

    for expected in &events {
        let received = rx.try_recv().ok();
        assert!(received.is_some(), "missing event: {expected:?}");
    }
}

#[test]
fn multiple_subscribers_all_receive() {
    let bus = EventBus::new();
    let rx1 = bus.subscribe();
    let rx2 = bus.subscribe();
    let id = NoteId::new();
    bus.try_publish(Event::NoteRemoved { id });
    assert!(rx1.try_recv().is_ok());
    assert!(rx2.try_recv().is_ok());
}

#[test]
fn dead_subscriber_is_pruned() {
    let bus = EventBus::new();
    let rx = bus.subscribe();
    let id = NoteId::new();
    drop(rx);
    // Should not panic when publishing to a dropped receiver
    bus.try_publish(Event::NoteRemoved { id });
}

#[test]
fn subscribe_after_dropped_still_works() {
    let bus = EventBus::new();
    let id = NoteId::new();
    {
        let rx = bus.subscribe();
        bus.try_publish(Event::NoteRemoved { id });
        assert!(rx.try_recv().is_ok());
    }
    // New subscriber after old one dropped
    let rx2 = bus.subscribe();
    bus.try_publish(Event::NoteRemoved { id });
    assert!(rx2.try_recv().is_ok());
}
