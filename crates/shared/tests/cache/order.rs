use super::Collision;
use shared::cache::{Manager, Policy};

#[derive(Default)]
struct Reference(Vec<(u8, u32)>);
impl Reference {
    fn remove(&mut self, key: u8) -> Option<u32> {
        let index = self.0.iter().position(|entry| entry.0 == key)?;
        Some(self.0.remove(index).1)
    }
    fn get(&mut self, key: u8) -> Option<u32> {
        let value = self.remove(key)?;
        self.0.push((key, value));
        Some(value)
    }
    fn resize(&mut self, capacity: usize) {
        let excess = self.0.len().saturating_sub(capacity);
        self.0.drain(..excess);
    }
    fn insert(&mut self, key: u8, value: u32, capacity: usize) -> bool {
        if capacity == 0 {
            return false;
        }
        self.remove(key);
        self.0.push((key, value));
        self.resize(capacity);
        true
    }
}

#[test]
fn differential() {
    let manager = Manager::default();
    let mut capacity = 7;
    let mut cache = manager
        .register("order", Policy::bounded(capacity, capacity))
        .unwrap();
    let mut reference = Reference::default();
    let mut random = 1709_u32;
    for step in 0..4096 {
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        let key = ((random >> 8) % 17) as u8;
        match random % 8 {
            0..=2 => assert_eq!(
                cache.insert(Collision(key), step, 1),
                reference.insert(key, step, capacity)
            ),
            3 | 4 => assert_eq!(cache.get(&Collision(key)), reference.get(key)),
            5 => assert_eq!(cache.remove(&Collision(key)), reference.remove(key)),
            6 => {
                capacity = key as usize % 11;
                cache
                    .configure(Policy::bounded(capacity, capacity))
                    .unwrap();
                reference.resize(capacity);
            }
            _ => {
                cache.clear();
                reference.0.clear();
            }
        }
        assert_eq!(cache.stats().entries, reference.0.len());
        assert_eq!(cache.stats().bytes, reference.0.len());
    }
    for key in 0..17 {
        assert_eq!(cache.get(&Collision(key)), reference.get(key));
    }
}

#[test]
fn disabled() {
    let manager = Manager::default();
    let mut cache = manager.register("order", Policy::bounded(3, 3)).unwrap();
    for key in 0..3 {
        cache.insert(key, key, 1);
    }
    for _ in 0..8 {
        assert_eq!(cache.get(&1), Some(1));
    }
    let mut policy = cache.policy();
    policy.read = false;
    cache.configure(policy).unwrap();
    assert_eq!(cache.get(&0), None);
    assert!(!cache.insert(0, 99, 4));
    assert!(cache.insert(3, 3, 1));
    policy.read = true;
    policy.write = false;
    cache.configure(policy).unwrap();
    assert_eq!(cache.get(&0), None);
    assert!(!cache.insert(2, 99, 1));
    cache.configure(Policy::bounded(1, 1)).unwrap();
    assert_eq!(cache.get(&3), Some(3));
    assert_eq!(cache.get(&1), None);
    cache.remove(&3);
    cache.insert(4, 4, 1);
    assert_eq!(cache.get(&4), Some(4));
}
