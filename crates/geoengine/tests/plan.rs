use geoengine::{
    Edge, Entity, Facts, Graph, Id, Impact, Plan, Relation, Request, Role, Source, Topology,
};
use std::sync::Arc;

fn entity(id: Id, version: u64, parent: Option<Id>, flow: std::ops::Range<usize>) -> Entity {
    Entity::new(
        (id, version),
        (parent, Source::new(1, id.value(), 1)),
        (Role::Source, false),
        flow,
    )
}

fn facts(identity: (u64, Id), graph: Graph) -> Arc<Facts> {
    Facts::new(identity.0, identity.1, graph).unwrap()
}

#[test]
fn propagation() {
    let root = Id::fresh();
    let first = Id::fresh();
    let second = Id::fresh();
    let edges = vec![
        Edge {
            from: first,
            to: root,
            relation: Relation::Intrinsic,
        },
        Edge {
            from: first,
            to: second,
            relation: Relation::Flow,
        },
    ];
    let previous = facts(
        (1, root),
        Graph {
            entities: vec![
                entity(root, 1, None, 0..2),
                entity(first, 1, Some(root), 2..2),
                entity(second, 1, Some(root), 2..2),
            ]
            .into(),
            edges: edges.clone().into(),
            flows: vec![1, 2].into(),
        },
    );
    let current = facts(
        (2, root),
        Graph {
            entities: vec![
                entity(root, 1, None, 0..2),
                entity(first, 2, Some(root), 2..2),
                entity(second, 1, Some(root), 2..2),
            ]
            .into(),
            edges: edges.into(),
            flows: vec![1, 2].into(),
        },
    );
    let plan = Plan::build(Request {
        current,
        previous: Some(previous),
        constraints: false,
        demand: None,
        prior: None,
    });
    assert_eq!(plan.topology(), Topology::Same);
    assert_eq!(plan.seeds(), [first]);
    assert_eq!(plan.affected().as_ref(), &[root, first, second]);
    assert!(plan.collection().is_some());
    let impact = Impact::audit(plan, &[root, first]);
    assert!(impact.missed().is_empty());
    assert_eq!(impact.excess(), [second]);
}

#[test]
fn remapping() {
    let root = Id::fresh();
    let child = Id::fresh();
    let previous = facts(
        (1, root),
        Graph {
            entities: vec![
                entity(root, 1, None, 0..1),
                entity(child, 1, Some(root), 1..1),
            ]
            .into(),
            edges: Arc::from([]),
            flows: vec![1].into(),
        },
    );
    let current = facts(
        (2, root),
        Graph {
            entities: vec![
                entity(child, 1, Some(root), 1..1),
                entity(root, 1, None, 0..1),
            ]
            .into(),
            edges: Arc::from([]),
            flows: vec![0].into(),
        },
    );
    let plan = Plan::build(Request {
        current,
        previous: Some(previous),
        constraints: false,
        demand: None,
        prior: None,
    });
    assert_eq!(plan.topology(), Topology::Remapped);
    assert!(plan.scope().is_some());
    assert!(plan.collection().is_none());
}

#[test]
fn alignment() {
    let root = Id::fresh();
    let child = Id::fresh();
    let entities = vec![
        entity(root, 1, None, 0..1),
        entity(child, 1, Some(root), 1..1),
    ];
    let previous = facts(
        (1, root),
        Graph {
            entities: entities.clone().into(),
            edges: Arc::from([]),
            flows: vec![1].into(),
        },
    );
    let current = facts(
        (2, root),
        Graph {
            entities: entities.into(),
            edges: vec![Edge {
                from: child,
                to: root,
                relation: Relation::Intrinsic,
            }]
            .into(),
            flows: vec![1].into(),
        },
    );
    let plan = Plan::build(Request {
        current,
        previous: Some(previous),
        constraints: false,
        demand: None,
        prior: None,
    });
    assert_eq!(plan.topology(), Topology::Aligned);
    assert!(plan.scope().is_some());
    assert!(plan.collection().is_none());
}

#[test]
fn boundaryclosure() {
    let root = Id::fresh();
    let boundary = Id::fresh();
    let child = Id::fresh();
    let make = |version| {
        let mut boundaryentity = entity(boundary, 1, Some(root), 1..2);
        boundaryentity.boundary = true;
        facts(
            (version, root),
            Graph {
                entities: vec![
                    entity(root, 1, None, 0..1),
                    boundaryentity,
                    entity(child, version, Some(boundary), 2..2),
                ]
                .into(),
                edges: vec![
                    Edge {
                        from: boundary,
                        to: root,
                        relation: Relation::Intrinsic,
                    },
                    Edge {
                        from: child,
                        to: boundary,
                        relation: Relation::Intrinsic,
                    },
                ]
                .into(),
                flows: vec![1, 2].into(),
            },
        )
    };
    let plan = Plan::build(Request {
        current: make(2),
        previous: Some(make(1)),
        constraints: false,
        demand: None,
        prior: None,
    });
    assert_eq!(plan.affected().as_ref(), &[boundary, child]);
    assert_eq!(plan.publication().as_ref(), &[root, boundary, child]);
}
