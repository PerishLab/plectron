use super::reference::{fill, frame, group, rect};
use graphics::shared::{
    Backend, Cpu, Effect, Effects, Frame, Item, Scene, Visual, operation::Source,
};

fn source(scene: &Scene, items: Vec<Item>) -> Source {
    Source {
        extent: scene.extent(),
        revision: scene.revision() + 1,
        items,
    }
}

#[test]
fn opacity() {
    let original = frame(vec![group(1, 255, vec![fill(rect(0, 0, 8, 8), 0xff0000)])]);
    let pixels = Cpu.render(&original).unwrap();
    for opacity in [128, 0, 255] {
        let mut items = original.scene().items().to_vec();
        let Item::Group { opacity: value, .. } = &mut items[0] else {
            unreachable!()
        };
        *value = opacity;
        let expected = Scene::new(original.extent(), 2, items.clone()).unwrap();
        let scene = original
            .scene()
            .revise(source(original.scene(), items), graphics::shared::CHECKS)
            .unwrap();
        assert_eq!(scene.routing().retained, 1);
        assert_eq!(scene.routing().checked, 1);
        let visual = original.visual().clone();
        let revised = original.revise(scene, visual.clone()).unwrap();
        assert!(std::ptr::eq(original.mappings(), revised.mappings()));
        assert_ne!(original.id(), revised.id());
        assert_eq!(
            Cpu.render(&revised).unwrap(),
            Cpu.render(&Frame::new(expected, visual).unwrap()).unwrap()
        );
    }
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn effect() {
    let original = frame(vec![group(
        1,
        255,
        vec![group(2, 255, vec![fill(rect(0, 0, 8, 8), 0xff0000)])],
    )]);
    let update = Effects::new(
        original.scene().generation(),
        vec![Effect {
            space: 2,
            opacity: 0,
        }],
    )
    .unwrap();
    let scene = original.scene().animate(&update).unwrap().unwrap();
    assert_eq!(scene.revision(), original.scene().revision());
    assert_eq!(scene.routing().retained, 1);
    assert_eq!(scene.routing().checked, 1);
    assert_eq!(scene.generation(), original.scene().generation());
    let frame = original.revise(scene, original.visual().clone()).unwrap();
    assert!(std::ptr::eq(original.mappings(), frame.mappings()));
    assert_eq!(Cpu.render(&frame).unwrap().pixels[0], 0xffffff);
    let invalid = Effects::new(
        frame.scene().generation(),
        vec![Effect {
            space: 99,
            opacity: 0,
        }],
    )
    .unwrap();
    assert!(frame.scene().animate(&invalid).is_err());
    let replaced = frame
        .scene()
        .revise(
            source(frame.scene(), frame.scene().items().to_vec()),
            graphics::shared::CHECKS,
        )
        .unwrap();
    assert_ne!(replaced.generation(), frame.scene().generation());
    assert!(replaced.animate(&update).unwrap().is_none());
}

#[test]
fn fallback() {
    let original = frame(vec![group(1, 255, vec![fill(rect(0, 0, 8, 8), 0xff0000)])]);
    let items = original.scene().items().to_vec();
    let scene = original
        .scene()
        .revise(source(original.scene(), items.clone()), 0)
        .unwrap();
    assert_eq!(scene.routing().retained, 0);
    let revised = original.revise(scene, original.visual().clone()).unwrap();
    assert!(!std::ptr::eq(original.mappings(), revised.mappings()));
    for translation in [[2, 0], [1_048_577, 0]] {
        let mut changed = items.clone();
        let Item::Group { space, .. } = &mut changed[0] else {
            unreachable!()
        };
        space.translation = translation;
        let actual = original
            .scene()
            .revise(source(original.scene(), changed), graphics::shared::CHECKS);
        if translation[0] == 2 {
            let scene = actual.unwrap();
            assert_eq!(scene.routing().retained, 0);
            assert_eq!(
                original
                    .revise(scene, original.visual().clone())
                    .unwrap()
                    .mapping(1)
                    .unwrap()
                    .translation,
                [2, 0]
            );
        } else {
            assert!(actual.is_err());
        }
    }
    let mut invalid = items.clone();
    invalid.extend(items);
    assert!(
        original
            .scene()
            .revise(source(original.scene(), invalid), graphics::shared::CHECKS)
            .is_err()
    );
    let mut visual = Visual::new(original.extent());
    visual.offsets.insert(1, 1);
    assert!(original.revise(original.scene().clone(), visual).is_err());
}

#[test]
fn sparse() {
    let original = frame(vec![group(
        1,
        255,
        (2..512)
            .map(|id| group(id, 255, vec![fill(rect(0, 0, 1, 1), 0)]))
            .collect(),
    )]);
    let mut items = original.scene().items().to_vec();
    let Item::Group { opacity, .. } = &mut items[0] else {
        unreachable!()
    };
    *opacity = 128;
    let scene = original
        .scene()
        .revise(source(original.scene(), items), 1)
        .unwrap();
    assert_eq!(scene.routing().checked, 1);
    assert_eq!(scene.routing().retained, 1);
    let revised = original.revise(scene, original.visual().clone()).unwrap();
    assert!(std::ptr::eq(original.mappings(), revised.mappings()));
    let mut changed = original.scene().items().to_vec();
    let Item::Group { items, .. } = &mut changed[0] else {
        unreachable!()
    };
    let mut children = items.to_vec();
    children.push(group(512, 255, Vec::new()));
    *items = children.into();
    let scene = original
        .scene()
        .revise(source(original.scene(), changed), 1)
        .unwrap();
    assert_eq!(scene.routing().retained, 0);
}

#[test]
fn validation() {
    let original = frame(vec![group(1, 255, vec![fill(rect(0, 0, 1, 1), 0)])]);
    for color in [0xffffff, 0x1000000] {
        let changed = vec![group(1, 0, vec![fill(rect(0, 0, 1, 1), color)])];
        let result = original
            .scene()
            .revise(source(original.scene(), changed), graphics::shared::CHECKS);
        if color == 0xffffff {
            assert_eq!(result.unwrap().routing().retained, 0);
        } else {
            assert!(result.is_err());
        }
    }
    let mut nested = Vec::new();
    for id in 1..40 {
        nested = vec![group(id, 0, nested)];
    }
    assert!(
        original
            .scene()
            .revise(source(original.scene(), nested), graphics::shared::CHECKS)
            .is_err()
    );
    let mut changed = source(original.scene(), original.scene().items().to_vec());
    changed.extent.width = 0;
    assert!(
        original
            .scene()
            .revise(changed, graphics::shared::CHECKS)
            .is_err()
    );
}

#[test]
fn impact() {
    let original = frame(vec![
        group(1, 255, vec![group(2, 255, vec![group(3, 255, Vec::new())])]),
        group(4, 255, Vec::new()),
    ]);
    let mut nested = original.visual().clone();
    nested.offsets.insert(1, 1);
    nested.offsets.insert(2, 1);
    assert_eq!(
        original.impact(original.scene(), &nested),
        graphics::shared::Impact {
            offsets: 2,
            affected: 3,
        }
    );
    let mut siblings = original.visual().clone();
    siblings.offsets.insert(2, 1);
    siblings.offsets.insert(4, 1);
    assert_eq!(
        original.impact(original.scene(), &siblings),
        graphics::shared::Impact {
            offsets: 2,
            affected: 3,
        }
    );
    let mut root = original.visual().clone();
    root.offsets.insert(0, 1);
    assert_eq!(original.impact(original.scene(), &root).affected, 5);

    let mut metadata = original.visual().clone();
    metadata.revision += 1;
    assert_eq!(original.impact(original.scene(), &metadata).affected, 0);
    let revised = original.revise(original.scene().clone(), metadata).unwrap();
    assert!(std::ptr::eq(original.mappings(), revised.mappings()));
    let mut equivalent = original.visual().clone();
    equivalent.offsets.insert(1, 0);
    assert_eq!(original.impact(original.scene(), &equivalent).affected, 0);
    let revised = original
        .revise(original.scene().clone(), equivalent)
        .unwrap();
    assert!(std::ptr::eq(original.mappings(), revised.mappings()));
}

#[test]
fn deferred() {
    let mut scrolling = group(1, 255, vec![group(2, 255, Vec::new())]);
    let Item::Group { space, .. } = &mut scrolling else {
        unreachable!()
    };
    space.clip = Some(rect(0, 0, 8, 8).into());
    space.scroll = Some(8);
    let original = frame(vec![scrolling]);
    assert!(original.projected());
    assert_eq!(
        original.materialization(),
        graphics::shared::Materialization::default()
    );
    let mut visual = original.visual().clone();
    visual.offsets.insert(1, 1);
    visual.revision += 1;
    let revised = original.revise(original.scene().clone(), visual).unwrap();
    assert!(!revised.projected());
    assert_eq!(revised.mapping(2).unwrap().translation, [0, -1]);
    assert!(revised.projected());
    assert_eq!(revised.materialization().materializations, 1);
    assert_eq!(revised.materialization().spaces, 3);
    assert_eq!(revised.materialization().queries, 1);
    assert_eq!(revised.materialization().traversals, 0);
    assert!(revised.materialization().elapsed > 0);
    let mut visual = revised.visual().clone();
    visual.offsets.insert(1, 2);
    visual.revision += 1;
    let next = revised.revise(revised.scene().clone(), visual).unwrap();
    assert!(!next.projected());
    assert_eq!(next.mappings()[2].translation, [0, -2]);
    assert_eq!(next.materialization().materializations, 2);
    assert_eq!(next.materialization().queries, 1);
    assert_eq!(next.materialization().traversals, 1);
}
