use crate::d3d12::{
    D3d12,
    pool::{Policy, Pool},
};
use crate::shared::Extent;
use std::time::Duration;

fn extent() -> Extent {
    Extent {
        width: 17,
        height: 13,
    }
}

fn empty(d3d12: &mut D3d12) -> crate::d3d12::output::Signal {
    let (allocator, list) = crate::d3d12::memory::Allocator(&d3d12.device)
        .command()
        .unwrap();
    let signal = crate::d3d12::output::commit(
        (&d3d12.device, &d3d12.queue),
        &list,
        (&d3d12.fence, &mut d3d12.value),
    )
    .unwrap();
    signal.wait(Duration::from_secs(5)).unwrap();
    let _keep = (allocator, list);
    signal
}

#[test]
fn completion() {
    let mut d3d12 = D3d12::new().unwrap();
    let mut pool = Pool::default();
    let first = pool.acquire(&d3d12.device, extent()).unwrap();
    let second = pool.acquire(&d3d12.device, extent()).unwrap();
    assert_ne!(first.resource, second.resource);
    let (allocator, list) = crate::d3d12::memory::Allocator(&d3d12.device)
        .command()
        .unwrap();
    let signal = crate::d3d12::output::Signal {
        device: d3d12.device.clone(),
        fence: d3d12.fence.clone(),
        value: d3d12.value + 1,
    };
    pool.submit(signal.clone());
    assert_eq!(pool.inspect().pending.entries, 2);
    let third = pool.acquire(&d3d12.device, extent()).unwrap();
    assert_ne!(third.resource, first.resource);
    assert_ne!(third.resource, second.resource);
    assert_eq!(pool.inspect().reuses, 0);
    pool.discard();
    let committed = crate::d3d12::output::commit(
        (&d3d12.device, &d3d12.queue),
        &list,
        (&d3d12.fence, &mut d3d12.value),
    )
    .unwrap();
    assert_eq!(committed.value, signal.value);
    committed.wait(Duration::from_secs(5)).unwrap();
    let _keep = (allocator, list);
    assert_eq!(pool.inspect().idle.entries, 2);
    let reused = pool.acquire(&d3d12.device, extent()).unwrap();
    assert!(reused.resource == first.resource || reused.resource == second.resource);
    assert_eq!(pool.inspect().reuses, 1);
    assert_eq!(pool.inspect().checked.entries, 1);
    pool.discard();
    assert_eq!(pool.inspect().checked.entries, 0);
    assert_eq!(pool.inspect().retired, 2);
}

#[test]
fn retirement() {
    let mut d3d12 = D3d12::new().unwrap();
    for closed in [false, true] {
        let mut pool = Pool::default();
        let texture = pool.acquire(&d3d12.device, extent()).unwrap();
        let (allocator, list) = crate::d3d12::memory::Allocator(&d3d12.device)
            .command()
            .unwrap();
        let pending = crate::d3d12::output::Signal {
            device: d3d12.device.clone(),
            fence: d3d12.fence.clone(),
            value: d3d12.value + 1,
        };
        pool.submit(pending);
        if closed {
            pool.destroy().unwrap();
        } else {
            pool.clear().unwrap();
        }
        assert_eq!(pool.inspect().pending.entries, 1);
        let committed = crate::d3d12::output::commit(
            (&d3d12.device, &d3d12.queue),
            &list,
            (&d3d12.fence, &mut d3d12.value),
        )
        .unwrap();
        committed.wait(Duration::from_secs(5)).unwrap();
        let _keep = (allocator, list);
        assert_eq!(pool.inspect().idle.entries, 0);
        assert_eq!(pool.inspect().retired, 1);
        let next = pool.acquire(&d3d12.device, extent()).unwrap();
        assert_ne!(texture.resource, next.resource);
        let signal = empty(&mut d3d12);
        pool.submit(signal);
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
    let mut d3d12 = D3d12::new().unwrap();
    let mut pool = Pool::default();
    let first = pool.acquire(&d3d12.device, extent()).unwrap();
    pool.acquire(&d3d12.device, extent()).unwrap();
    let signal = empty(&mut d3d12);
    pool.submit(signal);
    pool.configure(Policy {
        entries: 1,
        bytes: usize::MAX,
    })
    .unwrap();
    assert_eq!(pool.inspect().idle.entries, 1);
    assert_eq!(pool.inspect().idle.bytes, first.allocated);
    pool.configure(Policy {
        entries: 32,
        bytes: first.allocated - 1,
    })
    .unwrap();
    assert_eq!(pool.inspect().idle.entries, 0);
    assert_eq!(pool.inspect().retired, 2);
    let before = pool.inspect();
    assert!(
        pool.acquire(
            &d3d12.device,
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
    let mut d3d12 = D3d12::new().unwrap();
    let (allocator, list) = crate::d3d12::memory::Allocator(&d3d12.device)
        .command()
        .unwrap();
    let signal = crate::d3d12::output::Signal {
        device: d3d12.device.clone(),
        fence: d3d12.fence.clone(),
        value: d3d12.value + 1,
    };
    let completion = crate::d3d12::output::Completion::attach(signal, None);
    assert!(completion.wait(Duration::ZERO).is_err());
    let committed = crate::d3d12::output::commit(
        (&d3d12.device, &d3d12.queue),
        &list,
        (&d3d12.fence, &mut d3d12.value),
    )
    .unwrap();
    completion.wait(Duration::from_secs(5)).unwrap();
    completion.clone().wait(Duration::ZERO).unwrap();
    assert_eq!(committed.state(), crate::shared::State::Complete);
    let _keep = (allocator, list);
}

#[test]
fn abandon() {
    use crate::shared::{Backend, Cpu, Frame, Item, Rect, Scene, Visual};
    let mut d3d12 = D3d12::new().unwrap();
    let area = Extent {
        width: 64,
        height: 48,
    };
    let frame = Frame::new(
        Scene::new(
            area,
            0,
            vec![Item::Fill {
                rect: Rect {
                    x: 4,
                    y: 4,
                    width: 32,
                    height: 24,
                },
                color: Some(0x336699),
            }],
        )
        .unwrap(),
        Visual::new(area),
    )
    .unwrap();
    let dropped = d3d12.render(&frame).unwrap();
    drop(dropped);
    let follow = d3d12.render(&frame).unwrap();
    d3d12.wait(&follow, Duration::from_secs(5)).unwrap();
    assert_eq!(d3d12.read(&follow).unwrap(), Cpu.render(&frame).unwrap());
}
