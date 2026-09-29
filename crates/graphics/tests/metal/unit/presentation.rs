use crate::metal::{Metal, output, runtime::presentation};
use crate::shared::{Backend, Extent, Item, Rect, Scene, State};
use objc2::rc::autoreleasepool;
use objc2_foundation::NSSize;
use objc2_metal::{MTLPixelFormat, MTLResource};
use objc2_quartz_core::CAMetalLayer;
use std::time::{Duration, Instant};

fn scene() -> crate::shared::Frame {
    record(
        Scene::new(
            Extent {
                width: 31,
                height: 23,
            },
            0,
            vec![Item::Fill {
                rect: Rect {
                    x: 2,
                    y: 3,
                    width: 19,
                    height: 11,
                },
                color: Some(0x123456),
            }],
        )
        .unwrap(),
    )
}

#[test]
fn copy() {
    autoreleasepool(|_| {
        let mut metal = Metal::new().unwrap();
        let scene = scene();
        let source = metal.render(&scene).unwrap();
        let blank = record(Scene::new(scene.extent(), 0, Vec::new()).unwrap());
        let target = metal.render(&blank).unwrap();
        metal.wait(&source, Duration::from_secs(5)).unwrap();
        metal.wait(&target, Duration::from_secs(5)).unwrap();
        let command = presentation::copy(&source, &target.texture).unwrap();
        output::commit(&command).unwrap();
        assert_eq!(
            metal.read(&target).unwrap(),
            crate::shared::Cpu.render(&scene).unwrap()
        );
    });
}

#[test]
fn worker() {
    autoreleasepool(|_| {
        let mut metal = Metal::new().unwrap();
        let work = metal.render(&scene()).unwrap();
        metal.wait(&work, Duration::from_secs(5)).unwrap();
        let layer = CAMetalLayer::new();
        layer.setDevice(Some(&work.texture.device()));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(false);
        layer.setDrawableSize(NSSize::new(
            work.extent.width as f64,
            work.extent.height as f64,
        ));
        layer.setAllowsNextDrawableTimeout(true);
        let mut worker = presentation::Worker::new(layer, None, false).unwrap();
        worker.present(&work).unwrap();
        assert!(worker.present(&work).is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match worker.poll().unwrap() {
                State::Pending => assert!(Instant::now() < deadline),
                State::Complete => break,
                State::Failed(fault) => panic!("{fault}"),
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(worker.completed().is_some());
        assert_eq!(worker.visibility(), crate::shared::Visibility::Unobserved);
        assert!(worker.poll().is_err());
    });
}

fn record(scene: Scene) -> crate::shared::Frame {
    let visual = crate::shared::Visual::new(scene.extent());
    crate::shared::Frame::new(scene, visual).unwrap()
}
