use geoengine::{Block, Child, Flex, Point, Size, Unit};

#[test]
fn arithmetic() {
    assert_eq!(Unit::new(-2).offset(Unit::new(5)).unwrap().value(), 3);
    assert!(Unit::new(i32::MAX).offset(Unit::new(1)).is_err());
    assert!(Unit::new(i32::MIN).difference(Unit::new(1)).is_err());
    assert!(Unit::positive(i32::MAX as usize + 1).is_err());
    assert_eq!(Size::positive([3, 5]).unwrap().array(), Some([3, 5]));
    assert_eq!(
        Point {
            x: Unit::new(-1),
            y: Unit::ZERO,
        }
        .array(),
        None
    );
}

#[test]
fn distribution() {
    let item = || Flex::new(20, [0, 100], [1.0, 1.0], 0);
    let mut equal = [item(), item()];
    assert_eq!(
        geoengine::flex(&mut equal, Some(50), 0, 100),
        Ok(vec![25, 25])
    );

    let mut bounded = [
        Flex::new(20, [0, 22], [1.0, 1.0], 0),
        Flex::new(20, [0, 100], [1.0, 1.0], 0),
    ];
    assert_eq!(
        geoengine::flex(&mut bounded, Some(50), 0, 100),
        Ok(vec![22, 28])
    );

    let mut limited = [item(), item()];
    assert!(geoengine::flex(&mut limited, Some(50), 0, 40).is_err());
}

#[test]
fn blocks() {
    let child = |outer, coverage| {
        Child::new(
            Size::positive(outer).unwrap(),
            Size::positive(coverage).unwrap(),
        )
    };
    let flow = Block::new(
        vec![child([20, 10], [24, 12]), child([30, 15], [32, 18])],
        Unit::positive(32).unwrap(),
    )
    .resolve()
    .unwrap();
    assert_eq!(flow.size().array(), Some([30, 25]));
    assert_eq!(flow.overflow().array(), Some([32, 28]));
    assert_eq!(flow.placements()[1].array(), Some([0, 10]));
    assert!(
        Block::new(
            vec![child([20, 10], [20, 10]), child([20, 15], [20, 15])],
            Unit::positive(24).unwrap(),
        )
        .resolve()
        .is_err()
    );
}
