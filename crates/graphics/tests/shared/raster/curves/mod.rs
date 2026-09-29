use graphics::shared::{Clip, Geometry, Path, Rect, Scalar, Segment, Transform};

fn path() -> Path {
    Path::new(vec![
        Segment::Move([2.into(), 3.into()]),
        Segment::Quad([29.into(), (-9).into()], [24.into(), 27.into()]),
        Segment::Quad([(-8).into(), 37.into()], [2.into(), 3.into()]),
        Segment::Close,
        Segment::Move([7.into(), 9.into()]),
        Segment::Line([9.into(), 19.into()]),
        Segment::Quad([19.into(), 23.into()], [20.into(), 8.into()]),
        Segment::Close,
    ])
    .unwrap()
}
fn reference(curve: &graphics::shared::Curve, y: f64) -> Option<f64> {
    let [p, q, r] = curve.points.map(|p| p.map(f64::from));
    if y < p[1] || y >= r[1] {
        return None;
    }
    if !curve.quadratic {
        return Some(p[0] + (y - p[1]) / (r[1] - p[1]) * (r[0] - p[0]));
    }
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..48 {
        let t = (low + high) / 2.0;
        let ordinate = (1.0 - t) * (1.0 - t) * p[1] + 2.0 * t * (1.0 - t) * q[1] + t * t * r[1];
        if ordinate <= y {
            low = t;
        } else {
            high = t;
        }
    }
    let t = (low + high) / 2.0;
    Some((1.0 - t) * (1.0 - t) * p[0] + 2.0 * t * (1.0 - t) * q[0] + t * t * r[0])
}
fn samples(coverage: &Geometry, x: usize, y: usize) -> u8 {
    let mut count = 0;
    for row in 0..4 {
        for column in 0..4 {
            let point = [
                x as f32 + (column as f32 + 0.5) / 4.0,
                y as f32 + (row as f32 + 0.5) / 4.0,
            ];
            let winding: i32 = coverage
                .curves()
                .iter()
                .map(|curve| {
                    curve.direction
                        * i32::from(
                            reference(curve, f64::from(point[1]))
                                .is_some_and(|cross| cross > f64::from(point[0])),
                        )
                })
                .sum();
            count += u8::from(coverage.clip().contains(point) && winding != 0);
        }
    }
    count
}
#[test]
fn oracle() {
    for scale in [0.25, 1.0, 1.25, 2.0] {
        let coverage = Geometry::quadratic(
            &path(),
            Transform([
                Scalar::new(scale),
                Scalar::new(0.17),
                Scalar::new(-0.11),
                Scalar::new(scale),
                Scalar::new(5.137),
                Scalar::new(4.213),
            ]),
            Clip::from(Rect {
                x: 3,
                y: 2,
                width: 47,
                height: 49,
            })
            .scale(1.25),
        )
        .unwrap();
        assert!(coverage.edges().is_empty());
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(
                    coverage.samples(x, y),
                    samples(&coverage, x, y),
                    "pixel {x},{y}, scale {scale}"
                );
            }
        }
    }
}
#[test]
fn endpoints() {
    for controls in [
        [0.0, 0.0, 16.0],
        [0.0, 16.0, 16.0],
        [0.0, 8.0, 16.0],
        [16.0, 0.0, 16.0],
        [16.0, 32.0, 16.0],
    ] {
        let path = Path::new(vec![
            Segment::Move([0.into(), Scalar::new(controls[0])]),
            Segment::Quad(
                [16.into(), Scalar::new(controls[1])],
                [8.into(), Scalar::new(controls[2])],
            ),
            Segment::Close,
        ])
        .unwrap();
        let coverage = Geometry::quadratic(
            &path,
            Transform::default(),
            Rect {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            }
            .into(),
        )
        .unwrap();
        for curve in coverage.curves() {
            assert_eq!(curve.crossing(curve.points[2][1]), None);
            assert_eq!(curve.crossing(curve.points[0][1]), Some(curve.points[0][0]));
            roots(curve);
        }
    }
}
#[test]
fn boundaries() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    };
    let far = Path::new(vec![
        Segment::Move([(-200).into(), (-200).into()]),
        Segment::Line([1000.into(), (-200).into()]),
        Segment::Line([1000.into(), 1000.into()]),
        Segment::Line([(-200).into(), 1000.into()]),
        Segment::Close,
    ])
    .unwrap();
    let coverage = Geometry::quadratic(&far, Transform::default(), area.into()).unwrap();
    assert_eq!(coverage.samples(12, 12), 16);
    let empty = Geometry::quadratic(
        &Path::new(vec![]).unwrap(),
        Transform::default(),
        area.into(),
    )
    .unwrap();
    assert_eq!(empty.samples(12, 12), 0);
    assert_eq!(empty.visits(), 0);
    assert!(
        Geometry::quadratic(
            &far,
            Transform([
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                Scalar::new(20_000_000.0),
                0.into()
            ]),
            area.into()
        )
        .is_err()
    );
}

fn roots(curve: &graphics::shared::Curve) {
    for row in 0..128 {
        let y = row as f32 / 4.0;
        if let Some(expected) = reference(curve, f64::from(y)) {
            assert!((f64::from(curve.crossing(y).unwrap()) - expected).abs() < 0.00001);
        }
    }
}

#[test]
fn parabola() {
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([10.into(), 20.into()], [20.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    let clip = Rect {
        x: 0,
        y: 0,
        width: 24,
        height: 24,
    };
    let coverage = Geometry::quadratic(&path, Transform::default(), clip.into()).unwrap();
    assert_eq!(coverage.curves().len(), 2);
    for y in 0..24 {
        for x in 0..24 {
            let expected = (0..16)
                .filter(|sample| {
                    let u = x as f64 + ((sample % 4) as f64 + 0.5) / 4.0;
                    let v = y as f64 + ((sample / 4) as f64 + 0.5) / 4.0;
                    u > 0.0 && u < 20.0 && v < 2.0 * u - u * u / 10.0
                })
                .count();
            assert_eq!(usize::from(coverage.samples(x, y)), expected);
        }
    }
}
#[test]
fn budget() {
    let mut segments = vec![Segment::Move([0.into(), 0.into()])];
    segments
        .extend((0..32_769).map(|i| Segment::Line([((i + 1) % 2).into(), ((i + 1) % 2).into()])));
    segments.push(Segment::Close);
    let path = Path::new(segments).unwrap();
    let clip = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    assert!(Geometry::quadratic(&path, Transform::default(), clip.into()).is_err());
}

mod coverage;
mod metadata;
mod topology;
