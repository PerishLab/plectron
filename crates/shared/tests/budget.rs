use shared::budget::Budget;
use std::sync::{Barrier, mpsc};
use std::time::Duration;

#[test]
fn isolation() {
    assert!(Budget::new(0).acquire().is_none());
    let first = Budget::new(1);
    let second = Budget::new(1);
    let permit = first.acquire().unwrap();
    assert!(first.acquire().is_none());
    let other = second.acquire().unwrap();
    drop(permit);
    assert!(first.acquire().is_some());
    assert!(second.acquire().is_none());
    drop(other);
    assert!(second.acquire().is_some());
}
#[test]
fn contention() {
    let budget = Budget::new(4);
    let barrier = Barrier::new(17);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let sender = &sender;
            let budget = &budget;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                let permit = budget.acquire();
                sender.send(permit.is_some()).unwrap();
                barrier.wait();
                drop(permit);
            });
        }
        barrier.wait();
        let admitted = (0..16)
            .map(|_| receiver.recv_timeout(Duration::from_secs(5)).unwrap())
            .filter(|admitted| *admitted)
            .count();
        let exhausted = budget.acquire().is_none();
        barrier.wait();
        assert_eq!(admitted, 4);
        assert!(exhausted);
    });
    let permits: Vec<_> = (0..4).map(|_| budget.acquire().unwrap()).collect();
    assert!(budget.acquire().is_none());
    drop(permits);
    assert!(budget.acquire().is_some());
}
#[test]
fn retirement() {
    let budget = std::sync::Arc::new(Budget::new(1));
    let owner = budget.clone();
    let (started, active) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (finished, retired) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let permit = owner.acquire().unwrap();
        started.send(()).unwrap();
        wait.recv().unwrap();
        drop(permit);
        finished.send(()).unwrap();
    });
    active.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(thread);
    assert!(budget.acquire().is_none());
    release.send(()).unwrap();
    retired.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(budget.acquire().is_some());
}
#[test]
fn unwinding() {
    let budget = Budget::new(1);
    assert!(
        std::panic::catch_unwind(|| {
            let _permit = budget.acquire().unwrap();
            panic!("worker failed");
        })
        .is_err()
    );
    let permit = budget.acquire().unwrap();
    let unstarted = move || drop(permit);
    assert!(budget.acquire().is_none());
    drop(unstarted);
    assert!(budget.acquire().is_some());
}
