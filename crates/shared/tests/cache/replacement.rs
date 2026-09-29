use shared::cache::{Cost, Manager, Policy, Resource};
use std::sync::Arc;

#[test]
fn identity() {
    let manager = Manager::default();
    let mut cache = manager
        .register("replacement", Policy::bounded(2, 8))
        .unwrap();
    let key = Arc::new(1);
    let weak = Arc::downgrade(&key);
    cache.retain(key, 10, Cost::new(4));
    cache.insert(2, 20, 4);
    assert!(cache.replace(&1, 30, Cost::new(4)).is_ok());
    assert!(weak.upgrade().is_some());
    cache.insert(3, 40, 4);
    assert_eq!(cache.get(&2), None);
    assert_eq!(cache.get(&1), Some(30));
    assert_eq!(cache.stats().bytes, 8);
    cache.close();
    assert!(weak.upgrade().is_none());
}
fn charged(resource: Resource) -> Cost {
    let mut cost = Cost::new(2);
    cost.resource(resource);
    cost
}
#[test]
fn accounting() {
    let manager = Manager::default();
    let mut cache = manager
        .register("replacement", Policy::bounded(2, 12))
        .unwrap();
    let a = Resource::new(4);
    let b = Resource::new(4);
    cache.store(1, 1, charged(a.clone()));
    cache.store(2, 2, charged(a));
    assert!(cache.replace(&1, 3, charged(b)).is_ok());
    assert_eq!(cache.stats().bytes, 12);
    assert_eq!(cache.stats().resources, 2);
    let before = cache.stats();
    assert!(cache.replace(&2, 4, charged(Resource::new(8))).is_err());
    assert_eq!(cache.stats(), before);
    assert_eq!(cache.get(&2), Some(2));
}
#[test]
fn rejection() {
    let manager = Manager::default();
    let mut cache = manager
        .register("replacement", Policy::bounded(1, 4))
        .unwrap();
    assert!(cache.replace(&1, 2, Cost::new(4)).is_err());
    cache.insert(1, 1, 4);
    assert!(cache.replace(&1, 2, Cost::new(5)).is_err());
    cache
        .configure(Policy {
            write: false,
            ..Policy::bounded(1, 4)
        })
        .unwrap();
    assert!(cache.replace(&1, 2, Cost::new(4)).is_err());
    assert_eq!(cache.get(&1), Some(1));
    cache.close();
    assert!(cache.replace(&1, 2, Cost::new(4)).is_err());
}

#[test]
fn borrowing() {
    struct Value(usize);
    let manager = Manager::default();
    let mut cache = manager.register("borrowed", Policy::bounded(2, 8)).unwrap();
    cache.insert(1, Value(10), 4);
    cache.insert(2, Value(20), 4);
    assert_eq!(cache.lookup(&1).unwrap().0, 10);
    assert_eq!(cache.stats().requests, 1);
    assert_eq!(cache.stats().hits, 1);
    cache.insert(3, Value(30), 4);
    assert!(cache.lookup(&2).is_none());
    assert_eq!(cache.stats().evicted, 1);
    assert!(cache.replace(&1, Value(40), Cost::new(4)).is_ok());
    assert_eq!(cache.lookup(&1).unwrap().0, 40);
    cache
        .configure(Policy {
            read: false,
            ..Policy::bounded(2, 8)
        })
        .unwrap();
    let hits = cache.stats().hits;
    assert!(cache.lookup(&1).is_none());
    assert_eq!(cache.stats().hits, hits);
    cache.configure(Policy::bounded(2, 8)).unwrap();
    assert_eq!(cache.lookup(&1).unwrap().0, 40);
    manager
        .command(cache.id(), shared::cache::Command::Clear)
        .unwrap();
    cache.maintain();
    assert!(cache.lookup(&1).is_none());
    cache.insert(1, Value(50), 4);
    cache.close();
    assert!(cache.lookup(&1).is_none());
}
