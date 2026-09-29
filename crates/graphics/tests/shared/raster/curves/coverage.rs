use graphics::shared::{
    Clip, Coverage, Evaluation, Geometry, Path, Rect, Scalar, Segment, Transform, coverage::Mask,
};
use std::collections::BTreeMap;

fn compare(geometry: &Geometry) {
    let coverage = Coverage::new(geometry).unwrap();
    let mut actual = BTreeMap::new();
    for strip in coverage.strips() {
        assert!(strip.width > 0);
        for offset in 0..strip.width {
            let count = match &strip.mask {
                Mask::Solid => 16,
                Mask::Samples(range) => coverage.samples()[range.start + offset],
            };
            assert!((1..=16).contains(&count));
            assert!(actual.insert((strip.x + offset, strip.y), count).is_none());
        }
    }
    let rect = geometry.bounds;
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            assert_eq!(
                actual.get(&(x, y)).copied().unwrap_or(0),
                geometry.samples(x, y),
                "pixel {x},{y}"
            );
        }
    }
}
#[test]
fn oracle() {
    for evaluation in [Evaluation::Linear, Evaluation::Quadratic] {
        for scale in [0.25, 1.0, 1.25, 2.0] {
            let transform = Transform([
                Scalar::new(scale),
                Scalar::new(0.17),
                Scalar::new(-0.11),
                Scalar::new(scale),
                Scalar::new(5.137),
                Scalar::new(4.213),
            ]);
            let clip = Clip::from(Rect {
                x: 3,
                y: 2,
                width: 47,
                height: 49,
            })
            .scale(1.25);
            let geometry = prepare(evaluation, transform, clip);
            compare(&geometry);
        }
    }
}
#[test]
fn precision() {
    for origin in [0, 2_097_152, 8_388_608] {
        let clip = Rect {
            x: origin,
            y: origin,
            width: 12,
            height: 8,
        };
        let geometry = Geometry::rectangle(clip.into()).unwrap();
        compare(&geometry);
    }
    let geometry = Geometry::rectangle(
        Clip::from(Rect {
            x: 1,
            y: 3,
            width: 19,
            height: 13,
        })
        .scale(1.25),
    )
    .unwrap();
    compare(&geometry);
}
#[test]
fn sparse() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 4096,
        height: 1,
    };
    let geometry = Geometry::rectangle(area.into()).unwrap();
    let coverage = Coverage::new(&geometry).unwrap();
    assert_eq!(coverage.strips().len(), 1);
    assert_eq!(coverage.strips()[0].width, 4096);
    assert!(coverage.samples().is_empty());
    assert_eq!(coverage.work().visits, 8);
    assert_eq!(geometry.visits(), 4096 * 16 * 2);
    let empty = Geometry::clipped(
        &Path::new(vec![]).unwrap(),
        Transform::default(),
        area.into(),
    )
    .unwrap();
    assert!(Coverage::new(&empty).unwrap().strips().is_empty());
}
#[test]
fn crossing() {
    let path = Path::new(vec![
        Segment::Move([(-200).into(), (-200).into()]),
        Segment::Line([1000.into(), (-200).into()]),
        Segment::Line([1000.into(), 1000.into()]),
        Segment::Line([(-200).into(), 1000.into()]),
        Segment::Close,
        Segment::Move([8.into(), 8.into()]),
        Segment::Line([8.into(), 24.into()]),
        Segment::Line([24.into(), 24.into()]),
        Segment::Line([24.into(), 8.into()]),
        Segment::Close,
    ])
    .unwrap();
    let clip = Rect {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    };
    compare(&Geometry::clipped(&path, Transform::default(), clip.into()).unwrap());
    compare(&Geometry::quadratic(&path, Transform::default(), clip.into()).unwrap());
}

fn prepare(evaluation: Evaluation, transform: Transform, clip: Clip) -> Geometry {
    match evaluation {
        Evaluation::Linear => Geometry::clipped(&super::path(), transform, clip).unwrap(),
        Evaluation::Quadratic => Geometry::quadratic(&super::path(), transform, clip).unwrap(),
    }
}

#[test]
fn admission() {
    let geometry = Geometry::rectangle(
        Rect {
            x: 0,
            y: 0,
            width: 4_194_305,
            height: 1,
        }
        .into(),
    )
    .unwrap();
    assert!(Coverage::new(&geometry).is_err());
    let contour = [
        Segment::Move([0.into(), 0.into()]),
        Segment::Line([8.into(), 0.into()]),
        Segment::Line([8.into(), 4096.into()]),
        Segment::Line([0.into(), 4096.into()]),
        Segment::Close,
    ];
    let path = Path::new((0..8192).flat_map(|_| contour.clone()).collect()).unwrap();
    let geometry = Geometry::new(
        &path,
        Transform::default(),
        Rect {
            x: 1,
            y: 0,
            width: 1,
            height: 4096,
        },
    )
    .unwrap();
    assert!(Coverage::new(&geometry).is_err());
}
