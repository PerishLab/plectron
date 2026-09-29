use geoengine::{
    Bounds, Delta, Entity, Facts, Graph, Id, Layout, Metric, Point, Role, Size, Source, State,
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

fn state(id: Id, offset: [usize; 2], size: [usize; 2]) -> State {
    let origin = Point::positive(offset).unwrap();
    let size = Size::positive(size).unwrap();
    State::Resolved(Metric::new(
        id,
        origin,
        (
            Bounds::new(size, size, false),
            None,
            geoengine::Quality::Measured,
        ),
    ))
}

fn store(states: Vec<State>) -> geoengine::Store<State> {
    let mut builder = geoengine::Store::build(states.len(), None).unwrap();
    for (slot, state) in states.into_iter().enumerate() {
        builder.write(slot, state).unwrap();
    }
    builder.finish().unwrap()
}

fn facts(revision: u64, root: Id, entities: Vec<Entity>, flows: Vec<usize>) -> Arc<Facts> {
    Facts::new(
        revision,
        root,
        Graph {
            entities: entities.into(),
            edges: Arc::from([]),
            flows: flows.into(),
        },
    )
    .unwrap()
}

#[test]
fn changes() {
    let root = Id::fresh();
    let child = Id::fresh();
    let entities = vec![entity(root, None, 0..1), entity(child, Some(root), 1..1)];
    let previous = Facts::new(
        1,
        root,
        Graph {
            entities: entities.clone().into(),
            edges: Arc::from([]),
            flows: vec![1].into(),
        },
    )
    .unwrap();
    let current = Facts::new(
        2,
        root,
        Graph {
            entities: entities.into(),
            edges: Arc::from([]),
            flows: vec![1].into(),
        },
    )
    .unwrap();
    let old = store(vec![
        state(root, [0, 0], [20, 20]),
        state(child, [0, 0], [10, 10]),
    ]);
    let next = store(vec![
        state(root, [0, 0], [20, 20]),
        state(child, [5, 0], [12, 10]),
    ]);
    let delta = Delta::build(&current, &next, Some((&previous, &old)), None);
    assert_eq!(delta.changed(), [child]);
    assert_eq!(delta.moved(), [child]);
    assert_eq!(delta.resized(), [child]);
    let scoped = Delta::build(&current, &next, Some((&previous, &old)), Some(&[root]));
    assert!(scoped.changed().is_empty());
    assert_eq!(scoped.survey().compared, 1);
    assert_eq!(scoped.survey().scanned, 0);
    let oldlayout = Layout::new(previous, old, None, None).unwrap();
    let layout = Layout::new(current, next, Some(&oldlayout), None).unwrap();
    assert_ne!(layout.generation(), oldlayout.generation());
    assert_eq!(layout.delta().changed(), [child]);
    assert_eq!(layout.root().entity().id, root);
    let signature = layout.root().signature().unwrap();
    assert!(!signature.deferred());
    assert_eq!(signature.size(), Size::positive([20, 20]).unwrap());
    assert_eq!(signature.children(), [layout.entry(child).unwrap().stamp()]);
    assert_eq!(
        layout.metric(child).unwrap().origin(),
        Point::positive([5, 0]).unwrap()
    );
    assert_eq!(
        layout
            .entry(root)
            .unwrap()
            .children()
            .map(|entry| entry.entity().id)
            .collect::<Vec<_>>(),
        [child]
    );
}

#[test]
fn alignmentvalidation() {
    let root = Id::fresh();
    let child = Id::fresh();
    let facts = Facts::new(
        1,
        root,
        Graph {
            entities: vec![entity(root, None, 0..1), entity(child, Some(root), 1..1)].into(),
            edges: Arc::from([]),
            flows: vec![1].into(),
        },
    )
    .unwrap();
    let short = store(vec![state(root, [0, 0], [10, 10])]);
    assert!(Layout::new(facts.clone(), short, None, None).is_err());
    let swapped = store(vec![
        state(child, [0, 0], [10, 10]),
        state(root, [0, 0], [5, 5]),
    ]);
    assert!(Layout::new(facts.clone(), swapped, None, None).is_err());
    let states = store(vec![
        state(root, [0, 0], [10, 10]),
        state(child, [0, 0], [5, 5]),
    ]);
    assert!(Layout::new(facts, states, None, Some(&[Id::fresh()])).is_err());
}

#[test]
fn contentidentity() {
    let root = Id::fresh();
    let child = Id::fresh();
    let topology = || vec![entity(root, None, 0..1), entity(child, Some(root), 1..1)];
    let original = Layout::new(
        facts(1, root, topology(), vec![1]),
        store(vec![
            state(root, [0, 0], [20, 20]),
            state(child, [0, 0], [10, 10]),
        ]),
        None,
        None,
    )
    .unwrap();
    assert_eq!(original.publication().scanned, 2);
    assert_eq!(original.publication().fresh, 2);
    assert_eq!(original.publication().children, 1);
    assert_eq!(original.publication().allocated, 0);
    let moved = Layout::new(
        facts(2, root, topology(), vec![1]),
        store(vec![
            state(root, [4, 8], [20, 20]),
            state(child, [5, 0], [10, 10]),
        ]),
        Some(&original),
        None,
    )
    .unwrap();
    assert_eq!(moved.publication().compared, 2);
    assert_eq!(moved.publication().retained, 2);
    assert_eq!(moved.publication().fresh, 0);
    assert_eq!(moved.root().stamp(), original.root().stamp());
    assert_eq!(
        moved.entry(child).unwrap().stamp(),
        original.entry(child).unwrap().stamp()
    );

    let resized = Layout::new(
        facts(3, root, topology(), vec![1]),
        store(vec![
            state(root, [4, 8], [20, 20]),
            state(child, [5, 0], [11, 10]),
        ]),
        Some(&moved),
        None,
    )
    .unwrap();
    assert_eq!(resized.publication().fresh, 2);
    assert_ne!(
        resized.entry(child).unwrap().stamp(),
        moved.entry(child).unwrap().stamp()
    );
    assert_ne!(resized.root().stamp(), moved.root().stamp());
}

#[test]
fn identitysurvivesremapping() {
    let root = Id::fresh();
    let child = Id::fresh();
    let original = Layout::new(
        facts(
            1,
            root,
            vec![entity(root, None, 0..1), entity(child, Some(root), 1..1)],
            vec![1],
        ),
        store(vec![
            state(root, [0, 0], [20, 20]),
            state(child, [0, 0], [10, 10]),
        ]),
        None,
        None,
    )
    .unwrap();
    let remapped = Layout::new(
        facts(
            2,
            root,
            vec![entity(child, Some(root), 1..1), entity(root, None, 0..1)],
            vec![0],
        ),
        store(vec![
            state(child, [0, 0], [10, 10]),
            state(root, [0, 0], [20, 20]),
        ]),
        Some(&original),
        None,
    )
    .unwrap();
    assert_eq!(remapped.root().stamp(), original.root().stamp());
    assert_eq!(
        remapped.entry(child).unwrap().stamp(),
        original.entry(child).unwrap().stamp()
    );
}

#[test]
fn stateidentity() {
    let root = Id::fresh();
    let topology = || facts(1, root, vec![entity(root, None, 0..0)], vec![]);
    let resolved = Layout::new(
        topology(),
        store(vec![state(root, [0, 0], [20, 20])]),
        None,
        None,
    )
    .unwrap();
    let State::Resolved(metric) = state(root, [0, 0], [20, 20]) else {
        unreachable!()
    };
    let deferred = Layout::new(
        topology(),
        store(vec![State::Deferred(metric)]),
        Some(&resolved),
        None,
    )
    .unwrap();
    assert_ne!(deferred.root().stamp(), resolved.root().stamp());
    let blocked = Layout::new(
        topology(),
        store(vec![State::Blocked(root)]),
        Some(&deferred),
        None,
    )
    .unwrap();
    let repeated = Layout::new(
        topology(),
        store(vec![State::Blocked(root)]),
        Some(&blocked),
        None,
    )
    .unwrap();
    assert_ne!(blocked.root().stamp(), deferred.root().stamp());
    assert_eq!(repeated.root().stamp(), blocked.root().stamp());
}
