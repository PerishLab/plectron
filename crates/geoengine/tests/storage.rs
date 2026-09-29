use geoengine::Store;

#[test]
fn sharing() {
    let mut initial = Store::build(130, None).unwrap();
    assert_eq!(
        initial.traffic().materialized,
        130 * std::mem::size_of::<usize>()
    );
    for slot in 0..130 {
        initial.write(slot, slot).unwrap();
    }
    let initial = initial.finish().unwrap();
    let mut next = Store::build(130, Some(&initial)).unwrap();
    next.write(0, 200).unwrap();
    assert_eq!(next.traffic().copied, 16 * std::mem::size_of::<usize>());
    for slot in 1..130 {
        next.retain(slot).unwrap();
    }
    let next = next.finish().unwrap();
    assert_eq!(next[0], 200);
    assert_eq!(next[129], 129);
    assert_eq!(next.shared(&initial), 8);
    let residency = next.residency();
    assert_eq!(residency.logical, 130 * std::mem::size_of::<usize>());
    assert_eq!(residency.pages, 9);
    assert_eq!(residency.reused, 8);
}

#[test]
fn validation() {
    let mut builder = Store::build(2, None).unwrap();
    builder.write(0, 1).unwrap();
    assert!(builder.write(0, 2).is_err());
    assert!(builder.finish().is_err());
    assert!(Store::<usize>::paged(1, None, 0).is_err());
}

#[test]
fn pages() {
    let mut initial = Store::paged(130, None, 32).unwrap();
    for slot in 0..130 {
        initial.write(slot, slot).unwrap();
    }
    let initial = initial.finish().unwrap();
    let mut next = Store::paged(130, Some(&initial), 32).unwrap();
    next.write(0, 200).unwrap();
    assert_eq!(next.traffic().copied, 32 * std::mem::size_of::<usize>());
    for slot in 1..130 {
        next.retain(slot).unwrap();
    }
    let next = next.finish().unwrap();
    let residency = next.residency();
    assert_eq!(
        (residency.width, residency.pages, residency.reused),
        (32, 5, 4)
    );
    assert!(Store::paged(130, Some(&next), 64).is_err());
}
