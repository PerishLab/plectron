use super::reference::{fill, frame, group, rect};
use graphics::shared::{Backend, Cpu, Frame, Item, Output, Preparation};

fn rows() -> Frame {
    let rows = (0..8)
        .map(|index| {
            let mut row = group(
                index * 2 + 2,
                255,
                vec![group(
                    index * 2 + 3,
                    128,
                    vec![fill(rect(0, 0, 8, 8), 0x123456)],
                )],
            );
            if let Item::Group { space, .. } = &mut row {
                space.translation = [2, index as i64 * 16];
            }
            row
        })
        .collect();
    let mut root = group(1, 255, rows);
    if let Item::Group { space, .. } = &mut root {
        space.scroll = Some(128);
        space.clip = Some(rect(0, 0, 16, 16).into());
    }
    frame(vec![root])
}
fn verify(preparation: &mut Preparation, frame: &Frame, scale: f64) {
    let output = Output::new(frame.extent(), scale).unwrap();
    assert_eq!(
        Cpu.submit(&preparation.prepare(frame, output).unwrap())
            .unwrap(),
        Cpu.submit(&frame.prepare(output).unwrap()).unwrap()
    );
}
#[test]
fn scrolling() {
    let original = rows();
    let mut preparation = Preparation::default();
    for scale in [0.25, 1.0, 1.25, 2.0, 8.0] {
        for offset in [0, 3, 16, 112, 128, 0] {
            let mut visual = original.visual().clone();
            visual.offsets.insert(1, offset);
            let frame = Frame::new(original.scene().clone(), visual).unwrap();
            verify(&mut preparation, &frame, scale);
            assert!(preparation.work().pruned > 0);
            if offset != 0 {
                assert_eq!(preparation.work().bounded, 0);
            }
        }
    }
}
#[test]
fn policy() {
    let frame = rows();
    let config = config::Store::default();
    let mut preparation = Preparation::configured(config.clone());
    config.set("render.enclosure.items", 0.into());
    verify(&mut preparation, &frame, 1.0);
    assert_eq!(preparation.work().pruned, 0);
    config.remove("render.enclosure.items");
    verify(&mut preparation, &frame, 1.0);
    assert!(preparation.work().pruned > 0);
    config.set("render.subtrees", false.into());
    verify(&mut preparation, &frame, 1.0);
    assert_eq!(preparation.work().subtrees, 0);
    config.remove("render.subtrees");
    verify(&mut preparation, &frame, 1.0);
    assert!(preparation.work().pruned > 0);
    assert_eq!(preparation.work().bounded, 0);
}
#[test]
fn replacement() {
    let original = frame(vec![group(
        1,
        255,
        vec![group(2, 255, vec![fill(rect(0, 100, 8, 8), 0)])],
    )]);
    let changed = frame(vec![group(
        1,
        255,
        vec![group(2, 255, vec![fill(rect(0, 0, 8, 8), 0)])],
    )]);
    let mut preparation = Preparation::default();
    verify(&mut preparation, &original, 1.0);
    assert_eq!(preparation.work().pruned, 1);
    verify(&mut preparation, &changed, 1.0);
    assert_eq!(preparation.work().pruned, 0);
    assert!(preparation.work().bounded > 0);
    verify(&mut preparation, &original, 1.0);
    assert_eq!(preparation.work().pruned, 1);
    assert_eq!(preparation.work().bounded, 0);
}

#[test]
fn admission() {
    let frame = rows();
    let config = config::Store::default();
    let mut preparation = Preparation::configured(config.clone());
    verify(&mut preparation, &frame, 1.0);
    assert!(preparation.work().built > 0);
    assert!(preparation.work().pruned > 0);
    verify(&mut preparation, &frame, 1.0);
    assert_eq!(preparation.work().built, 0);
    assert!(preparation.work().pruned > 0);
    config.set("render.enclosure.items", 1.into());
    verify(&mut preparation, &frame, 1.0);
    assert!(preparation.work().built > 0);
    verify(&mut preparation, &frame, 1.0);
    assert_eq!(preparation.work().built, 0);
    assert!(preparation.work().unproven > 0);
    assert_eq!(preparation.work().pruned, 0);
}
