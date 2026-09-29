use graphics::shared::{Extent, Frame, Item, Rect, Scene, Space, Visual};

fn group(id: u64, scroll: Option<usize>, items: Vec<Item>) -> Item {
    Item::Group {
        space: Space {
            id,
            translation: [1, 2],
            clip: scroll.map(|_| {
                Rect {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                }
                .into()
            }),
            scroll,
        },
        opacity: 255,
        items: items.into(),
    }
}
fn frame(items: Vec<Item>) -> Frame {
    let extent = Extent {
        width: 64,
        height: 64,
    };
    Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap()
}
#[test]
fn ancestry() {
    let leaf = group(u64::MAX, None, vec![]);
    let old = frame(vec![group(
        81,
        Some(7),
        vec![group(2, None, vec![leaf.clone()])],
    )]);
    let changed = frame(vec![group(
        2,
        None,
        vec![group(81, None, vec![leaf.clone()])],
    )]);
    let isolated = frame(vec![group(81, Some(0), vec![leaf])]);
    for view in [&old, &changed, &isolated] {
        for mapping in view.mappings() {
            assert_eq!(view.mapping(mapping.id), Some(mapping));
            let mut expected = mapping;
            while expected.range.is_none() {
                expected = view.mapping(expected.parent.unwrap()).unwrap();
            }
            assert_eq!(view.scroller(mapping.id), Some(expected.id));
        }
        assert_eq!(view.scroller(0), Some(0));
        assert_eq!(view.scroller(99), None);
        assert_eq!(view.mapping(99), None);
    }
    assert_eq!(old.scroller(u64::MAX), Some(81));
    assert_eq!(changed.scroller(u64::MAX), Some(0));
    assert_eq!(isolated.scroller(u64::MAX), Some(81));
    assert_eq!(
        old.mappings().iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![0, 81, 2, u64::MAX]
    );
    let mut visual = old.visual().clone();
    visual.offsets.insert(81, 7);
    let scrolled = Frame::new(old.scene().clone(), visual).unwrap();
    assert_eq!(scrolled.scroller(u64::MAX), Some(81));
    assert_ne!(old.mapping(u64::MAX), scrolled.mapping(u64::MAX));
    drop(changed);
    assert_eq!(old.scroller(u64::MAX), Some(81));
}
#[test]
fn duplicates() {
    let extent = Extent {
        width: 64,
        height: 64,
    };
    assert!(
        Scene::new(
            extent,
            0,
            vec![group(u64::MAX, None, vec![]), group(u64::MAX, None, vec![])]
        )
        .is_err()
    );
}
