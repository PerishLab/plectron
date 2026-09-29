use crate::metal::{
    Metal,
    pool::{Policy, Pool},
};
use crate::shared::Extent;
use objc2_metal::{MTLCommandQueue, MTLResource};

fn extent() -> Extent {
    Extent {
        width: 17,
        height: 13,
    }
}
#[test]
fn completion() {
    let metal = Metal::new().unwrap();
    let mut pool = Pool::default();
    let first = pool.acquire(&metal.device, extent()).unwrap();
    let second = pool.acquire(&metal.device, extent()).unwrap();
    assert_ne!(*first, *second);
    let command = metal.queue.commandBuffer().unwrap();
    pool.submit(command.clone());
    assert_eq!(pool.inspect().pending.entries, 2);
    let third = pool.acquire(&metal.device, extent()).unwrap();
    assert_ne!(*third, *first);
    assert_ne!(*third, *second);
    assert_eq!(pool.inspect().reuses, 0);
    pool.discard();
    crate::metal::output::commit(&command).unwrap();
    assert_eq!(pool.inspect().idle.entries, 2);
    let reused = pool.acquire(&metal.device, extent()).unwrap();
    assert!(*reused == *first || *reused == *second);
    assert_eq!(pool.inspect().reuses, 1);
    assert_eq!(pool.inspect().checked.entries, 1);
    pool.discard();
    assert_eq!(pool.inspect().checked.entries, 0);
    assert_eq!(pool.inspect().retired, 2);
}
#[test]
fn retirement() {
    let metal = Metal::new().unwrap();
    for closed in [false, true] {
        let mut pool = Pool::default();
        let texture = pool.acquire(&metal.device, extent()).unwrap();
        let command = metal.queue.commandBuffer().unwrap();
        pool.submit(command.clone());
        if closed {
            pool.destroy().unwrap();
        } else {
            pool.clear().unwrap();
        }
        assert_eq!(pool.inspect().pending.entries, 1);
        crate::metal::output::commit(&command).unwrap();
        assert_eq!(pool.inspect().idle.entries, 0);
        assert_eq!(pool.inspect().retired, 1);
        let next = pool.acquire(&metal.device, extent()).unwrap();
        assert_ne!(*texture, *next);
        let command = metal.queue.commandBuffer().unwrap();
        pool.submit(command.clone());
        crate::metal::output::commit(&command).unwrap();
        assert_eq!(pool.inspect().idle.entries, usize::from(!closed));
        assert_eq!(pool.clear().is_err(), closed);
        assert_eq!(
            pool.configure(Policy {
                entries: 1,
                bytes: 4096
            })
            .is_err(),
            closed
        );
    }
}
#[test]
fn budgets() {
    let metal = Metal::new().unwrap();
    let mut pool = Pool::default();
    let first = pool.acquire(&metal.device, extent()).unwrap();
    pool.acquire(&metal.device, extent()).unwrap();
    let command = metal.queue.commandBuffer().unwrap();
    pool.submit(command.clone());
    pool.configure(Policy {
        entries: 1,
        bytes: usize::MAX,
    })
    .unwrap();
    crate::metal::output::commit(&command).unwrap();
    assert_eq!(pool.inspect().idle.entries, 1);
    assert_eq!(pool.inspect().idle.bytes, first.allocatedSize());
    pool.configure(Policy {
        entries: 32,
        bytes: first.allocatedSize() - 1,
    })
    .unwrap();
    assert_eq!(pool.inspect().idle.entries, 0);
    assert_eq!(pool.inspect().retired, 2);
    let before = pool.inspect();
    assert!(
        pool.acquire(
            &metal.device,
            Extent {
                width: 0,
                height: 1
            }
        )
        .is_err()
    );
    assert_eq!(pool.inspect(), before);
}

#[test]
fn notification() {
    use objc2_metal::MTLCommandBuffer;
    use std::time::Duration;
    let metal = Metal::new().unwrap();
    let command = metal.queue.commandBuffer().unwrap();
    let completion = crate::metal::output::Completion::attach(&command, None);
    assert!(completion.wait(Duration::ZERO).is_err());
    command.commit();
    completion.wait(Duration::from_secs(5)).unwrap();
    completion.clone().wait(Duration::ZERO).unwrap();
}
