use geoengine::{
    Attachment, Bounds, Entity, Facts, Graph, Id, Layout, Metric, Point, Role, Size, Source, State,
};
use std::sync::Arc;

fn entity(id: Id, parent: Option<Id>, flow: std::ops::Range<usize>) -> Entity {
    Entity::new(
        (id, 1),
        (parent, Source::new(1, id.value(), 1)),
        (Role::Source, false),
        flow,
    )
}

fn layout(
    id: Id,
    bounds: ([usize; 2], [usize; 2], bool),
    attachment: Option<Attachment>,
    previous: Option<&Layout>,
) -> Arc<Layout> {
    qualified(
        id,
        bounds,
        (attachment, geoengine::Quality::Measured),
        previous,
    )
}

fn qualified(
    id: Id,
    bounds: ([usize; 2], [usize; 2], bool),
    content: (Option<Attachment>, geoengine::Quality),
    previous: Option<&Layout>,
) -> Arc<Layout> {
    let (attachment, quality) = content;
    let facts = Facts::new(
        1,
        id,
        Graph {
            entities: vec![entity(id, None, 0..0)].into(),
            edges: Arc::from([]),
            flows: Arc::from([]),
        },
    )
    .unwrap();
    let (size, overflow, clipped) = bounds;
    let metric = Metric::new(
        id,
        Point::positive([0, 0]).unwrap(),
        (
            Bounds::new(
                Size::positive(size).unwrap(),
                Size::positive(overflow).unwrap(),
                clipped,
            ),
            attachment,
            quality,
        ),
    );
    let mut states = geoengine::Store::build(1, None).unwrap();
    states.write(0, State::Resolved(metric)).unwrap();
    Layout::new(facts, states.finish().unwrap(), previous, None).unwrap()
}

#[test]
fn qualityidentity() {
    let id = Id::fresh();
    let bounds = ([10, 10], [10, 10], true);
    let estimated = qualified(id, bounds, (None, geoengine::Quality::Estimated), None);
    let stale = qualified(
        id,
        bounds,
        (None, geoengine::Quality::Stale),
        Some(&estimated),
    );
    let measured = qualified(
        id,
        bounds,
        (None, geoengine::Quality::Measured),
        Some(&stale),
    );
    assert_ne!(estimated.root().stamp(), stale.root().stamp());
    assert_ne!(stale.root().stamp(), measured.root().stamp());
    assert_eq!(
        measured.root().signature().unwrap().quality(),
        geoengine::Quality::Measured
    );
}

#[test]
fn signaturefields() {
    let id = Id::fresh();
    let original = layout(id, ([10, 10], [10, 10], false), None, None);
    let overflow = layout(id, ([10, 10], [11, 10], false), None, Some(&original));
    let clipped = layout(id, ([10, 10], [10, 10], true), None, Some(&original));
    let attached = layout(
        id,
        ([10, 10], [10, 10], false),
        Some(Attachment::fresh()),
        Some(&original),
    );
    assert_ne!(overflow.root().stamp(), original.root().stamp());
    assert_ne!(clipped.root().stamp(), original.root().stamp());
    assert_ne!(attached.root().stamp(), original.root().stamp());
}

#[test]
fn cyclicflow() {
    let first = Id::fresh();
    let second = Id::fresh();
    let facts = Facts::new(
        1,
        first,
        Graph {
            entities: vec![
                entity(first, Some(second), 0..1),
                entity(second, Some(first), 1..2),
            ]
            .into(),
            edges: Arc::from([]),
            flows: vec![1, 0].into(),
        },
    );
    assert!(facts.is_err());
}

#[test]
fn closedflow() {
    let root = Id::fresh();
    let child = Id::fresh();
    let graph = |entities, flows| Graph {
        entities,
        edges: Arc::from([]),
        flows,
    };
    assert!(
        Facts::new(
            1,
            root,
            graph(
                vec![entity(root, None, 0..0), entity(child, Some(root), 0..0)].into(),
                Arc::from([]),
            ),
        )
        .is_err()
    );
    assert!(
        Facts::new(
            1,
            root,
            graph(
                vec![entity(root, None, 0..1), entity(child, None, 1..1)].into(),
                vec![1].into(),
            ),
        )
        .is_err()
    );
    assert!(
        Facts::new(
            1,
            root,
            graph(
                vec![entity(root, None, 0..2), entity(child, Some(root), 2..2)].into(),
                vec![1, 1].into(),
            ),
        )
        .is_err()
    );
}

#[test]
fn graphshape() {
    let root = Id::fresh();
    let branch = Id::fresh();
    let leaf = Id::fresh();
    let nested = Id::fresh();
    let facts = Facts::new(
        1,
        root,
        Graph {
            entities: vec![
                entity(root, None, 0..2),
                entity(branch, Some(root), 2..3),
                entity(leaf, Some(root), 3..3),
                entity(nested, Some(branch), 3..3),
            ]
            .into(),
            edges: Arc::from([]),
            flows: vec![1, 2, 3].into(),
        },
    )
    .unwrap();
    let shape = facts.shape();
    assert_eq!((shape.nodes, shape.flows, shape.roots), (4, 3, 1));
    assert_eq!((shape.waves, shape.width, shape.fanout), (3, 2, 2));
}
