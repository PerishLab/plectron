use shared::cache::{Command, Cost, Manager, Policy, Resource};
use std::sync::Arc;

#[test]
fn collections() {
    for count in [0, 1, 4, 5, 64] {
        let resources: Vec<_> = (0..count).map(|_| Resource::new(8)).collect();
        let mut first = Cost::new(2);
        for resource in &resources {
            first.resource(resource.clone());
            first.resource(resource.clone());
        }
        let mut second = first.clone();
        second.extend(&first);
        second.resource(Resource::new(8));
        assert_eq!(first.total(), 2 + count * 8);
        assert_eq!(second.total(), 4 + (count + 1) * 8);
        let manager = Manager::default();
        let mut cache = manager
            .register("collections", Policy::bounded(2, 4096))
            .unwrap();
        assert!(cache.store(1, (), first));
        assert!(cache.store(2, (), second));
        assert_eq!(cache.stats().resources, count + 1);
        assert_eq!(cache.stats().bytes, 6 + (count + 1) * 8);
        cache.remove(&2);
        assert_eq!(cache.stats().resources, count);
        assert_eq!(cache.stats().bytes, 2 + count * 8);
        cache.clear();
        assert_eq!(cache.stats().bytes, 0);
    }
}
fn cost(bytes: usize, resource: &Resource) -> Cost {
    let mut cost = Cost::new(bytes);
    cost.resource(resource.clone());
    cost.resource(resource.clone());
    cost
}
#[test]
fn retention() {
    let resource = Resource::new(8);
    let manager = Manager::default();
    let mut cache = manager.register("shared", Policy::bounded(4, 16)).unwrap();
    assert!(cache.store("a", Arc::new(1), cost(4, &resource)));
    assert!(cache.store("b", Arc::new(2), cost(4, &resource)));
    assert_eq!(
        (
            cache.stats().local,
            cache.stats().shared,
            cache.stats().resources
        ),
        (8, 8, 1)
    );
    let held = cache.get(&"a").unwrap();
    cache.remove(&"a");
    assert_eq!(cache.stats().bytes, 12);
    cache.remove(&"b");
    assert_eq!(cache.stats().bytes, 0);
    assert_eq!(cache.stats().resources, 0);
    assert_eq!(*held, 1);
    cache.store("a", Arc::new(3), cost(4, &resource));
    cache.configure(Policy::bounded(1, 8)).unwrap();
    assert_eq!(cache.stats().entries, 0);
    assert_eq!(cache.stats().shared, 0);
    cache.store("a", Arc::new(4), cost(0, &resource));
    manager.command(cache.id(), Command::Destroy).unwrap();
    cache.maintain();
    assert_eq!(cache.stats().resources, 0);
    assert!(!cache.store("b", Arc::new(5), cost(0, &resource)));
}
#[test]
fn eviction() {
    let a = Resource::new(8);
    let b = Resource::new(8);
    let mut cache = Manager::default()
        .register("shared", Policy::bounded(2, 20))
        .unwrap();
    cache.store(1, 1, cost(2, &a));
    cache.store(2, 2, cost(2, &b));
    let mut incoming = cost(3, &a);
    incoming.resource(b.clone());
    assert!(cache.store(3, 3, incoming));
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.stats().bytes, 19);
    assert_eq!(cache.stats().evicted, 2);
    assert!(!cache.store(3, 4, cost(21, &a)));
    assert_eq!(cache.get(&3), Some(3));
    assert!(cache.store(3, 5, cost(2, &a)));
    assert_eq!(cache.stats().bytes, 10);
    assert_eq!(cache.stats().resources, 1);
    cache.clear();
    assert_eq!(cache.stats().bytes, 0);
}
#[test]
fn isolation() {
    let resource = Resource::new(8);
    let manager = Manager::default();
    let mut a = manager.register("a", Policy::bounded(2, 20)).unwrap();
    let mut b = manager.register("b", Policy::bounded(2, 20)).unwrap();
    a.store(1, (), cost(1, &resource));
    b.store(1, (), cost(1, &resource));
    a.clear();
    assert_eq!(b.stats().bytes, 9);
    assert!(b.store(2, (), cost(1, &Resource::new(8))));
    assert_eq!(b.stats().resources, 2);
    assert_eq!(b.stats().bytes, 18);
}
#[test]
fn overflow() {
    let mut cache = Manager::default()
        .register("overflow", Policy::bounded(2, usize::MAX))
        .unwrap();
    let mut sum = Cost::new(usize::MAX);
    sum.extend(&Cost::new(1));
    assert!(!cache.store(1, (), sum));
    assert!(!cache.store(1, (), cost(1, &Resource::new(usize::MAX))));
    assert_eq!(cache.stats().bytes, 0);
    assert_eq!(cache.stats().overflowed, 2);
    assert_eq!(cache.stats().oversized, 0);
    assert_eq!(cache.stats().largest, 0);
}

#[test]
fn diagnostics() {
    let manager = Manager::default();
    let resource = Resource::new(12);
    let mut cache = manager
        .register("admission", Policy::bounded(2, 8))
        .unwrap();
    assert!(cache.insert(1, "old", 4));
    assert!(!cache.store(1, "replacement", cost(2, &resource)));
    assert_eq!(cache.get(&1), Some("old"));
    assert_eq!(cache.stats().oversized, 1);
    assert_eq!(cache.stats().largest, 14);
    cache.configure(Policy::bounded(2, 16)).unwrap();
    assert!(cache.store(1, "replacement", cost(2, &resource)));
    assert!(cache.store(2, "shared", cost(2, &resource)));
    assert_eq!(cache.stats().bytes, 16);
    assert_eq!(cache.stats().largest, 14);
    cache.configure(Policy::bounded(0, 0)).unwrap();
    assert!(!cache.insert(3, "blocked", usize::MAX));
    assert_eq!(cache.stats().largest, 14);
    cache.configure(Policy::bounded(1, 0)).unwrap();
    assert!(cache.insert(3, "empty", 0));
    assert!(!cache.insert(4, "oversized", 1));
    let mut policy = cache.policy();
    policy.write = false;
    cache.configure(policy).unwrap();
    assert!(!cache.insert(4, "disabled", 0));
    cache.clear();
    cache.close();
    assert!(!cache.insert(4, "closed", 0));
    cache.publish();
    let snapshot = &manager.inspect()[0];
    assert_eq!(snapshot.stats, cache.stats());
    assert_eq!(snapshot.stats.bytes, 0);
    assert_eq!(snapshot.stats.largest, 14);
    assert_eq!(snapshot.stats.blocked, 3);
    assert_eq!(snapshot.stats.oversized, 2);
    assert_eq!(snapshot.stats.overflowed, 0);
    assert_eq!(snapshot.stats.omitted, 5);
}
