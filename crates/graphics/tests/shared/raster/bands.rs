use graphics::shared::{Clip, Geometry, Path, Rect, Scalar, Segment, Transform};

fn rectangle(left: i64, top: i64, right: i64, bottom: i64) -> Vec<Segment> {
    vec![
        Segment::Move([left.into(), top.into()]),
        Segment::Line([right.into(), top.into()]),
        Segment::Line([right.into(), bottom.into()]),
        Segment::Line([left.into(), bottom.into()]),
        Segment::Close,
    ]
}
fn scan(coverage: &Geometry, point: [f32; 2]) -> bool {
    if !coverage.clip().contains(point) {
        return false;
    }
    let mut winding = 0;
    for &[x, y, u, v] in coverage.edges() {
        if (y <= point[1] && v > point[1]) || (v <= point[1] && y > point[1]) {
            let cross = x + (point[1] - y) / (v - y) * (u - x);
            if cross > point[0] {
                winding += i32::from(v > y) * 2 - 1;
            }
        }
    }
    winding != 0
}
fn reference(coverage: &Geometry, x: usize, y: usize) -> u8 {
    let mut count = 0;
    for row in 0..4 {
        for column in 0..4 {
            count += u8::from(scan(
                coverage,
                [
                    x as f32 + (column as f32 + 0.5) / 4.0,
                    y as f32 + (row as f32 + 0.5) / 4.0,
                ],
            ));
        }
    }
    count
}
fn compare(coverage: &Geometry, area: Rect) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            assert_eq!(
                coverage.samples(x, y),
                reference(coverage, x, y),
                "pixel {x},{y}"
            );
        }
    }
}
#[test]
fn winding() {
    let mut segments = rectangle(-200, -200, 1000, 1000);
    segments.extend(rectangle(8, 56, 56, 8));
    segments.extend([
        Segment::Move([16.into(), 16.into()]),
        Segment::Quad([64.into(), 16.into()], [32.into(), 56.into()]),
        Segment::Quad([(-16).into(), 48.into()], [16.into(), 16.into()]),
        Segment::Close,
    ]);
    let path = Path::new(segments).unwrap();
    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 80,
    };
    for scale in [0.25, 1.0, 1.25, 2.0] {
        let clip = Clip::from(Rect {
            x: 3,
            y: 5,
            width: 51,
            height: 49,
        })
        .scale(1.25);
        let transform = Transform([
            Scalar::new(scale),
            Scalar::ZERO,
            Scalar::ZERO,
            Scalar::new(scale),
            Scalar::new(-0.375),
            Scalar::new(0.125),
        ]);
        let coverage = Geometry::clipped(&path, transform, clip).unwrap();
        compare(&coverage, area);
        assert!(
            coverage
                .bands()
                .candidates(12)
                .iter()
                .any(|&index| coverage.edges()[index as usize][0] > 100.0)
        );
    }
}
#[test]
fn contours() {
    let mut seed = 71_u64;
    let mut segments = Vec::new();
    for _ in 0..32 {
        let mut point = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            Scalar::new(((seed >> 32) % 384) as f64 / 4.0 - 16.0)
        };
        segments.extend([
            Segment::Move([point(), point()]),
            Segment::Quad([point(), point()], [point(), point()]),
            Segment::Line([point(), point()]),
            Segment::Close,
        ]);
    }
    let area = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let coverage =
        Geometry::new(&Path::new(segments).unwrap(), Transform::default(), area).unwrap();
    compare(&coverage, area);
    let linear = (area.width * area.height * coverage.edges().len() * 16) as u64;
    assert!(coverage.visits() < linear / 2);
}
#[test]
fn endpoints() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 48,
    };
    let mut segments = rectangle(0, 0, 1, 48);
    for row in [0, 8, 16, 24, 32, 40] {
        segments.extend(rectangle(4, row, 32, row + 8));
    }
    let path = Path::new(segments).unwrap();
    for offset in [-0.125, 0.0, 0.125, 0.875] {
        let transform = Transform::default().translate([Scalar::ZERO, Scalar::new(offset)]);
        let coverage = Geometry::new(&path, transform, area).unwrap();
        compare(&coverage, area);
    }
}
#[test]
fn bounded() {
    let segments = (0..400).flat_map(|_| rectangle(0, 0, 50, 4096)).collect();
    let area = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 4096,
    };
    let coverage =
        Geometry::new(&Path::new(segments).unwrap(), Transform::default(), area).unwrap();
    assert!(coverage.bands().stride() > 64);
    assert!(coverage.bands().data().len() <= coverage.edges().len() * 8 + 65);
    for y in [0, 64, 1024, 4095] {
        assert_eq!(coverage.samples(10, y), reference(&coverage, 10, y));
    }
    assert!(coverage.bands().candidates(4096).is_empty());
    assert!(coverage.bands().candidates(usize::MAX).is_empty());
}
#[test]
fn empty() {
    let area = Rect {
        x: 4,
        y: 9,
        width: 16,
        height: 32,
    };
    for segments in [vec![], rectangle(0, 0, 8, 0), rectangle(40, 40, 50, 50)] {
        let coverage =
            Geometry::new(&Path::new(segments).unwrap(), Transform::default(), area).unwrap();
        compare(&coverage, area);
        assert_eq!(coverage.visits(), 0);
    }
    let coverage = Geometry::rectangle(Clip::from(area).scale(1.25)).unwrap();
    compare(
        &coverage,
        Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 64,
        },
    );
}

#[test]
fn precision() {
    let area = Rect {
        x: 0,
        y: 2_097_152,
        width: 8,
        height: 64,
    };
    let mut segments = rectangle(0, 0, 2, 64);
    for y in (8..56).step_by(16) {
        segments.extend(rectangle(2, y, 8, y + 8));
    }
    let coverage = Geometry::new(
        &Path::new(segments).unwrap(),
        Transform::default().translate([Scalar::ZERO, 2_097_152.into()]),
        area,
    )
    .unwrap();
    compare(&coverage, area);
}

#[test]
fn admission() {
    use graphics::shared::{Extent, Frame, Ink, Item, Output, Record, Scene, Visual};
    use std::sync::Arc;
    let extent = Extent {
        width: 512,
        height: 512,
    };
    for height in [2, 512] {
        let segments = (0..400)
            .flat_map(|index| {
                let top = index % 64 * 8;
                rectangle(0, top, 512, top + height)
            })
            .collect();
        let record = Arc::new(
            Record::new(Arc::new(Ink::Path {
                path: Path::new(segments).unwrap(),
                transform: Transform::default(),
                color: 0,
            }))
            .unwrap(),
        );
        let frame = Frame::new(
            Scene::new(
                extent,
                0,
                vec![Item::Draw {
                    origin: [0; 2],
                    clip: None,
                    record,
                }],
            )
            .unwrap(),
            Visual::new(extent),
        )
        .unwrap();
        let pass = frame.prepare(Output::new(extent, 1.0).unwrap());
        if height == 2 {
            let pass = pass.unwrap();
            let graphics::shared::Primitive::Path(coverage, _) = &pass.layer().primitives[0] else {
                panic!("path");
            };
            assert!(
                coverage.bounds.width * coverage.bounds.height * coverage.edges().len() * 16
                    > 1_000_000_000
            );
            assert!(coverage.visits() < 1_000_000_000);
        } else {
            assert!(
                pass.unwrap_err()
                    .to_string()
                    .contains("geometry work budget")
            );
        }
    }
}
