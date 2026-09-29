use shared::geometry::{Path, Segment};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

#[derive(Default)]
struct Collision;
impl Hasher for Collision {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}
#[derive(Default)]
struct Count(usize);
impl Hasher for Count {
    fn finish(&self) -> u64 {
        self.0 as u64
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }
}
fn path(count: usize) -> Path {
    let mut segments = vec![Segment::Move([0.into(); 2])];
    segments.extend((0..count).map(|_| Segment::Line([1.into(); 2])));
    segments.push(Segment::Close);
    Path::new(segments).unwrap()
}
#[test]
fn identity() {
    let original = path(4);
    let clone = original.clone();
    let rebuilt = Path::new(original.segments().to_vec()).unwrap();
    assert_eq!(original, rebuilt);
    assert!(std::ptr::eq(original.segments(), clone.segments()));
    let mut map = HashMap::<_, _, BuildHasherDefault<Collision>>::default();
    map.insert(original, 1);
    assert_eq!(map.insert(rebuilt, 2), Some(1));
    let distinct = path(5);
    assert_eq!(clone.bounds(), distinct.bounds());
    assert_ne!(clone, distinct);
    map.insert(distinct, 3);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&clone), Some(&2));
    drop(map);
    assert_eq!(clone.segments().len(), 6);
}
#[test]
fn hashing() {
    let small = path(4);
    let large = path(4096);
    let mut a = Count::default();
    let mut b = Count::default();
    small.hash(&mut a);
    large.hash(&mut b);
    assert_eq!(a.0, b.0);
    assert!(a.0 > 0);
    assert!(large.retained() > small.retained());
}
