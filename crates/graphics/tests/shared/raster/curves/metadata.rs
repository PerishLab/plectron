use graphics::shared::{
    Backend, Cpu, Evaluation, Extent, Frame, Ink, Item, Output, Path, Preparation, Rasterization,
    Record, Scene, Transform, Visual,
};
use std::sync::Arc;

fn frame(path: Path, transform: Transform) -> Frame {
    let extent = Extent {
        width: 80,
        height: 80,
    };
    let record = Record::new(Arc::new(Ink::Path {
        path,
        transform,
        color: 0x123456,
    }))
    .unwrap();
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Draw {
                origin: [0; 2],
                clip: None,
                record: Arc::new(record),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
#[test]
fn transforms() {
    for values in [
        [1.25, 0.0, 0.0, 0.75, 20.25, 18.5],
        [-1.0, 0.0, 0.0, 1.0, 40.25, 20.5],
        [1.0, 0.0, 0.0, -1.0, 20.25, 40.5],
        [0.0, 0.0, 0.0, 1.0, 20.25, 18.5],
        [0.0, 1.0, -1.0, 0.0, 40.25, 18.5],
        [1.0, 0.25, -0.5, 1.0, 20.25, 18.5],
    ] {
        let frame = frame(
            super::path(),
            Transform(values.map(graphics::shared::Scalar::new)),
        );
        let mut local = Preparation::default();
        local.evaluation(Evaluation::Quadratic);
        local.rasterization(Rasterization::Local);
        let mut direct = Preparation::default();
        direct.evaluation(Evaluation::Quadratic);
        let output = Output::new(frame.extent(), 1.0).unwrap();
        let local = local.prepare(&frame, output).unwrap();
        let direct = direct.prepare(&frame, output).unwrap();
        assert_eq!(
            Cpu.submit(&local).unwrap(),
            Cpu.submit(&direct).unwrap(),
            "{values:?}"
        );
    }
}
#[test]
fn wire() {
    let path = super::path();
    let value = serde_json::to_value(&path).unwrap();
    assert_eq!(value, serde_json::to_value(path.segments()).unwrap());
    let rebuilt: Path = serde_json::from_value(value).unwrap();
    assert_eq!(path, rebuilt);
    assert_eq!(path.bounds(), rebuilt.bounds());
    let mut map = std::collections::HashMap::new();
    map.insert(path, 1);
    assert_eq!(map.get(&rebuilt), Some(&1));
}
