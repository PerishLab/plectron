use shared::cache::{Command, Manager, Policy};
use std::sync::Arc;

#[test]
fn retention() {
    let manager = Manager::default();
    let mut cache = manager.register("test", Policy::bounded(2, 8)).unwrap();
    cache.insert("a", Arc::new(1), 3);
    cache.insert("b", Arc::new(2), 3);
    let held = cache.get(&"a").unwrap();
    cache.insert("c", Arc::new(3), 3);
    assert!(cache.get(&"b").is_none());
    assert_eq!(cache.stats().evicted, 1);
    assert!(!cache.insert("a", Arc::new(4), 9));
    assert_eq!(*cache.get(&"a").unwrap(), 1);
    assert!(cache.insert("a", Arc::new(5), 6));
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.stats().bytes, 6);
    assert_eq!(*held, 1);
    cache.publish();
    assert_eq!(manager.inspect()[0].stats, cache.stats());
    drop(cache);
    assert!(manager.inspect().is_empty());
    assert_eq!(*held, 1);
}

#[test]
fn lifecycle() {
    let manager = Manager::default();
    let mut cache = manager.register("test", Policy::bounded(4, 16)).unwrap();
    cache.insert(1, Arc::new("first"), 4);
    cache.insert(2, Arc::new("second"), 4);
    let held = cache.get(&1).unwrap();
    cache.publish();
    let id = cache.id();
    let control = manager.clone();
    let generation = std::thread::spawn(move || {
        control
            .command(
                id,
                Command::Configure {
                    policy: Policy::bounded(1, 4),
                },
            )
            .unwrap()
    })
    .join()
    .unwrap();
    let pending = &manager.inspect()[0];
    assert_eq!(pending.requested, generation);
    assert_eq!(pending.applied, 0);
    assert_eq!(pending.stats.entries, 2);
    cache.maintain();
    let applied = &manager.inspect()[0];
    assert_eq!(applied.applied, generation);
    assert_eq!(applied.stats.entries, 1);
    assert!(cache.get(&2).is_none());
    manager.command(id, Command::Clear).unwrap();
    manager
        .command(
            id,
            Command::Configure {
                policy: Policy::bounded(8, 32),
            },
        )
        .unwrap();
    cache.maintain();
    assert!(cache.get(&1).is_none());
    cache.insert(3, Arc::new("third"), 4);
    manager.command(id, Command::Destroy).unwrap();
    assert!(manager.command(id, Command::Clear).is_err());
    cache.maintain();
    assert!(manager.inspect()[0].closed);
    assert_eq!(cache.stats().bytes, 0);
    assert!(cache.get(&3).is_none());
    assert!(!cache.insert(4, Arc::new("fourth"), 4));
    assert_eq!(*held, "first");
    drop(cache);
    assert!(manager.command(id, Command::Clear).is_err());
    let next = manager
        .register::<u8, u8>("next", Policy::bounded(1, 1))
        .unwrap();
    assert!(next.id() > id);
}

#[test]
fn policies() {
    let manager = Manager::default();
    let mut cache = manager.register("test", Policy::bounded(2, 8)).unwrap();
    cache.insert(1, Arc::new(10), 4);
    let mut policy = cache.policy();
    policy.read = false;
    cache.configure(policy).unwrap();
    assert!(cache.get(&1).is_none());
    assert!(cache.insert(2, Arc::new(20), 4));
    policy.read = true;
    policy.write = false;
    cache.configure(policy).unwrap();
    assert_eq!(*cache.get(&2).unwrap(), 20);
    assert!(!cache.insert(3, Arc::new(30), 4));
    cache.configure(Policy::bounded(0, 0)).unwrap();
    assert_eq!(cache.stats().entries, 0);
    assert!(!cache.insert(1, Arc::new(1), 0));
    drop(manager);
    cache.configure(Policy::bounded(1, 1)).unwrap();
    assert!(cache.insert(1, Arc::new(1), 1));
}

#[derive(Clone, PartialEq, Eq)]
struct Collision(u8);
impl std::hash::Hash for Collision {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&0, state);
    }
}
#[test]
fn identity() {
    let manager = Manager::default();
    let mut cache = manager.register("test", Policy::bounded(2, 8)).unwrap();
    cache.insert(Collision(1), Arc::new(10), 4);
    cache.insert(Collision(2), Arc::new(20), 4);
    assert_eq!(*cache.get(&Collision(1)).unwrap(), 10);
    assert_eq!(*cache.get(&Collision(2)).unwrap(), 20);
    let mut registrations = Vec::new();
    for _ in 0..63 {
        registrations.push(
            manager
                .register::<u8, u8>("entry", Policy::bounded(1, 1))
                .unwrap(),
        );
    }
    assert!(
        manager
            .register::<u8, u8>("excess", Policy::bounded(1, 1))
            .is_err()
    );
    registrations.pop();
    assert!(
        manager
            .register::<u8, u8>("replacement", Policy::bounded(1, 1))
            .is_ok()
    );
}

#[test]
fn publication() {
    let manager = Manager::default();
    let mut cache = manager
        .register("test", Policy::bounded(2, usize::MAX))
        .unwrap();
    assert!(cache.insert(1, Arc::new(1), usize::MAX));
    assert!(cache.insert(2, Arc::new(2), 1));
    assert_eq!(cache.stats().bytes, 1);
    assert_eq!(cache.stats().evicted, 1);
    let requested = manager
        .command(
            cache.id(),
            Command::Configure {
                policy: Policy::bounded(0, 0),
            },
        )
        .unwrap();
    cache.publish();
    let pending = &manager.inspect()[0];
    assert_eq!(pending.requested, requested);
    assert_eq!(pending.applied, 0);
    assert_eq!(pending.target.entries, 0);
    assert_eq!(pending.active.entries, 2);
    assert_eq!(pending.stats.entries, 1);
    cache.maintain();
    assert_eq!(manager.inspect()[0].applied, requested);
    assert_eq!(cache.stats().bytes, 0);
}

#[test]
fn local() {
    let manager = Manager::default();
    let mut cache = manager.register("local", Policy::bounded(1, 1)).unwrap();
    let value = std::rc::Rc::new(std::cell::Cell::new(7));
    let weak = std::rc::Rc::downgrade(&value);
    assert!(cache.insert(1, value, 1));
    let held = cache.get(&1).unwrap();
    manager.command(cache.id(), Command::Clear).unwrap();
    cache.maintain();
    assert_eq!(cache.stats().entries, 0);
    drop(cache);
    assert!(manager.inspect().is_empty());
    assert_eq!(held.get(), 7);
    assert!(weak.upgrade().is_some());
    drop(held);
    assert!(weak.upgrade().is_none());
}

#[path = "cache/accounting.rs"]
mod accounting;

#[path = "cache/order.rs"]
mod order;

#[test]
fn keys() {
    use shared::cache::Cost;
    #[derive(PartialEq, Eq, Hash)]
    struct Key(usize);
    let manager = Manager::default();
    let mut cache = manager.register("keys", Policy::bounded(1, 8)).unwrap();
    let key = Arc::new(Key(1));
    let weak = Arc::downgrade(&key);
    let held = Arc::new("retained");
    assert!(cache.retain(key, held.clone(), Cost::new(4)));
    assert!(weak.upgrade().is_some());
    assert!(Arc::ptr_eq(&cache.get(&Key(1)).unwrap(), &held));
    assert!(!cache.retain(Arc::new(Key(1)), Arc::new("rejected"), Cost::new(9)));
    assert!(weak.upgrade().is_some());
    assert!(Arc::ptr_eq(&cache.get(&Key(1)).unwrap(), &held));
    assert!(cache.store(Key(1), Arc::new("replaced"), Cost::new(4)));
    assert!(weak.upgrade().is_none());
    let key = Arc::new(Key(2));
    let weak = Arc::downgrade(&key);
    assert!(cache.retain(key, held.clone(), Cost::new(4)));
    assert!(cache.get(&Key(1)).is_none());
    assert_eq!(cache.stats().evicted, 1);
    cache.close();
    assert!(weak.upgrade().is_none());
    assert_eq!(*held, "retained");
    assert_eq!(cache.stats().bytes, 0);
}

#[path = "cache/replacement.rs"]
mod replacement;
