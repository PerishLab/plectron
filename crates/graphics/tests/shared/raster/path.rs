use graphics::shared::{
    Extent, Geometry, Ink, Item, Path, Record, Rect, Scalar, Segment, Transform, raster,
};
use std::sync::Arc;

fn rectangle(left: i64, top: i64, right: i64, bottom: i64) -> Vec<Segment> {
    vec![
        Segment::Move([left.into(), top.into()]),
        Segment::Line([right.into(), top.into()]),
        Segment::Line([right.into(), bottom.into()]),
        Segment::Line([left.into(), bottom.into()]),
        Segment::Close,
    ]
}
fn clip() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    }
}

#[test]
fn winding() {
    let mut segments = rectangle(0, 0, 40, 40);
    segments.extend(rectangle(10, 30, 30, 10));
    let path = Path::new(segments).unwrap();
    let coverage = Geometry::new(&path, Transform::default(), clip()).unwrap();
    assert_eq!(coverage.samples(5, 5), 16);
    assert_eq!(coverage.samples(20, 20), 0);
    let mut segments = rectangle(0, 0, 40, 40);
    segments.extend(rectangle(10, 10, 30, 30));
    let coverage =
        Geometry::new(&Path::new(segments).unwrap(), Transform::default(), clip()).unwrap();
    assert_eq!(coverage.samples(20, 20), 16);
}
#[test]
fn fractional() {
    let path = Path::new(rectangle(-1, 0, 1, 2)).unwrap();
    let transform = Transform::default().translate([Scalar::new(0.5), Scalar::ZERO]);
    let coverage = Geometry::new(&path, transform, clip()).unwrap();
    assert_eq!(coverage.samples(0, 0), 16);
    assert_eq!(coverage.samples(1, 0), 8);
    let ink = Ink::Path {
        path,
        transform,
        color: 0,
    };
    let record = Record::new(Arc::new(ink)).unwrap();
    let wire = serde_json::to_string(&record).unwrap();
    let decoded: Record = serde_json::from_str(&wire).unwrap();
    let extent = Extent {
        width: 64,
        height: 64,
    };
    let image = raster(
        extent,
        &[Item::Draw {
            origin: [0; 2],
            clip: None,
            record: Arc::new(decoded),
        }],
    )
    .unwrap();
    assert_eq!(image.pixels[0], 0);
    assert_eq!(image.pixels[1], 0x7f7f7f);
    assert_eq!(image.pixels[2], 0xffffff);
}
#[test]
fn adaptive() {
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([50.into(), 100.into()], [100.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    let small = Transform([
        Scalar::new(0.1),
        0.into(),
        0.into(),
        Scalar::new(0.1),
        0.into(),
        0.into(),
    ]);
    let large = Transform([4.into(), 0.into(), 0.into(), 4.into(), 0.into(), 0.into()]);
    let small = Geometry::new(&path, small, clip()).unwrap();
    let large = Geometry::new(&path, large, clip()).unwrap();
    assert!(large.edges().len() > small.edges().len());
    assert_eq!(small.samples(5, 2), 16);
    assert_eq!(small.samples(5, 6), 0);
}
#[test]
fn admission() {
    assert!(Path::new(vec![Segment::Line([0.into(); 2])]).is_err());
    assert!(Path::new(vec![Segment::Move([0.into(); 2])]).is_err());
    assert!(serde_json::from_str::<Path>("[{\"Move\":[0,0]}]").is_err());
    let path = Path::new(rectangle(0, 0, 1, 1)).unwrap();
    let ink = Ink::Path {
        path,
        transform: Transform([Scalar::new(2_000_000.0); 6]),
        color: 0,
    };
    assert!(Record::new(Arc::new(ink)).is_err());
}

#[path = "curves/mod.rs"]
mod curves;
