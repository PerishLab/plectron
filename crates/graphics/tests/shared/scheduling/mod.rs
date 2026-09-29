mod pacing;
mod timing;
mod visibility;
use graphics::shared::{
    Backend, Driver, Extent, Fault, Image, Scene, Sink, State, Target, Visibility,
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

struct Record {
    drawing: Result<State, Fault>,
    presentation: State,
    visibility: Visibility,
    submitted: Vec<u64>,
    copied: Vec<u64>,
    resource: Rc<()>,
}

#[derive(Clone)]
struct Device(Rc<RefCell<Record>>);

impl Device {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Record {
            drawing: Ok(State::Pending),
            presentation: State::Pending,
            visibility: Visibility::Unobserved,
            submitted: Vec::new(),
            copied: Vec::new(),
            resource: Rc::new(()),
        })))
    }
}
impl Backend for Device {
    type Work = (u64, Rc<()>);
    fn submit(&mut self, scene: &graphics::shared::Pass) -> Result<Self::Work, Fault> {
        let mut record = self.0.borrow_mut();
        record.submitted.push(scene.id());
        Ok((scene.id(), record.resource.clone()))
    }
    fn poll(&mut self, _: &mut Self::Work) -> Result<State, Fault> {
        self.0.borrow().drawing.clone()
    }
    fn read(&mut self, _: &Self::Work) -> Result<Image, Fault> {
        unreachable!()
    }
}
struct Output {
    target: Target,
    device: Device,
}
impl Sink<(u64, Rc<()>)> for Output {
    fn target(&self) -> Target {
        self.target
    }
    fn present(&mut self, work: &(u64, Rc<()>)) -> Result<(), Fault> {
        self.device.0.borrow_mut().copied.push(work.0);
        Ok(())
    }
    fn poll(&mut self) -> Result<State, Fault> {
        Ok(self.device.0.borrow().presentation.clone())
    }
    fn visibility(&self) -> Visibility {
        self.device.0.borrow().visibility
    }
}
fn scene() -> graphics::shared::Frame {
    record(
        Scene::new(
            Extent {
                width: 2,
                height: 2,
            },
            0,
            Vec::new(),
        )
        .unwrap(),
    )
}
fn attach(driver: &mut Driver<Device, Output>, device: &Device) -> Target {
    let target = driver.target(scene().extent()).unwrap();
    driver
        .attach(Output {
            target,
            device: device.clone(),
        })
        .unwrap();
    target
}

#[test]
fn stages() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let target = attach(&mut driver, &device);
    let first = scene();
    driver.draw(target, first.clone()).unwrap();
    for _ in 0..10 {
        assert!(driver.poll(Instant::now()).is_empty());
    }
    assert_eq!(device.0.borrow().submitted, [first.id()]);
    assert!(device.0.borrow().copied.is_empty());
    let skipped = scene();
    let latest = scene();
    assert_ne!(first.id(), latest.id());
    assert_eq!(first.scene().revision(), latest.scene().revision());
    let displaced = skipped.id();
    assert_eq!(driver.draw(target, skipped).unwrap(), None);
    assert_eq!(
        driver.draw(target, latest.clone()).unwrap(),
        Some(displaced)
    );
    device.0.borrow_mut().drawing = Ok(State::Complete);
    for _ in 0..10 {
        assert!(driver.poll(Instant::now()).is_empty());
    }
    assert_eq!(device.0.borrow().copied, [first.id()]);
    assert_eq!(device.0.borrow().submitted, [first.id()]);
    device.0.borrow_mut().presentation = State::Complete;
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].frame, first.id());
    assert!(receipts[0].result.is_ok());
    assert_eq!(device.0.borrow().submitted, [first.id(), latest.id()]);
    assert!(driver.poll(Instant::now()).is_empty());
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].frame, latest.id());
    assert!(driver.poll(Instant::now()).is_empty());
}

#[test]
fn pressure() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let targets: Vec<_> = (0..8).map(|_| attach(&mut driver, &device)).collect();
    for target in &targets {
        driver.draw(*target, scene()).unwrap();
    }
    assert!(driver.poll(Instant::now()).is_empty());
    assert_eq!(device.0.borrow().submitted.len(), 2);
    device.0.borrow_mut().drawing = Ok(State::Complete);
    for _ in 0..8 {
        assert!(driver.poll(Instant::now()).is_empty());
    }
    assert_eq!(device.0.borrow().submitted.len(), 8);
    assert_eq!(device.0.borrow().copied.len(), 8);
    device.0.borrow_mut().presentation = State::Complete;
    assert_eq!(driver.poll(Instant::now()).len(), 8);
    assert_eq!(Rc::strong_count(&device.0.borrow().resource), 1);
}

#[test]
fn closing() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let target = attach(&mut driver, &device);
    driver.draw(target, scene()).unwrap();
    driver.poll(Instant::now());
    driver.close(target).unwrap();
    assert_eq!(Rc::strong_count(&device.0.borrow().resource), 2);
    assert!(driver.draw(target, scene()).is_err());
    device.0.borrow_mut().drawing = Ok(State::Complete);
    assert!(driver.poll(Instant::now()).is_empty());
    assert_eq!(Rc::strong_count(&device.0.borrow().resource), 1);
    assert!(device.0.borrow().copied.is_empty());
    let target = attach(&mut driver, &device);
    driver.draw(target, scene()).unwrap();
    driver.poll(Instant::now());
    driver.poll(Instant::now());
    driver.close(target).unwrap();
    device.0.borrow_mut().presentation = State::Complete;
    assert!(driver.poll(Instant::now()).is_empty());
}

#[test]
fn failures() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let first = attach(&mut driver, &device);
    let second = attach(&mut driver, &device);
    driver.draw(first, scene()).unwrap();
    driver.draw(second, scene()).unwrap();
    driver.poll(Instant::now());
    device.0.borrow_mut().drawing = Err(Fault("device lost".into()));
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 2);
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.result.as_ref().unwrap_err().0 == "device lost")
    );
    assert!(driver.draw(first, scene()).is_err());
    driver.close(first).unwrap();
    driver.close(second).unwrap();
    assert!(driver.poll(Instant::now()).is_empty());
}

#[test]
fn presentation() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let target = attach(&mut driver, &device);
    driver.draw(target, scene()).unwrap();
    driver.poll(Instant::now());
    device.0.borrow_mut().drawing = Ok(State::Complete);
    driver.poll(Instant::now());
    device.0.borrow_mut().presentation = State::Failed(Fault("drawable unavailable".into()));
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        receipts[0].result.as_ref().unwrap_err().0,
        "drawable unavailable"
    );
    assert!(driver.poll(Instant::now()).is_empty());
    assert!(driver.draw(target, scene()).is_err());
    driver.close(target).unwrap();
    let next = attach(&mut driver, &device);
    driver.draw(next, scene()).unwrap();
}

#[test]
fn independent() {
    let device = Device::new();
    let fast = Device::new();
    let mut driver = Driver::new(device.clone());
    let first = attach(&mut driver, &device);
    let second = attach(&mut driver, &fast);
    driver.draw(first, scene()).unwrap();
    driver.draw(second, scene()).unwrap();
    driver.poll(Instant::now());
    device.0.borrow_mut().drawing = Ok(State::Complete);
    driver.poll(Instant::now());
    fast.0.borrow_mut().presentation = State::Complete;
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].target, second);
    assert!(receipts[0].result.is_ok());
    assert!(driver.poll(Instant::now()).is_empty());
    driver.close(first).unwrap();
    driver.close(second).unwrap();
}

fn record(scene: Scene) -> graphics::shared::Frame {
    let visual = graphics::shared::Visual::new(scene.extent());
    graphics::shared::Frame::new(scene, visual).unwrap()
}

#[test]
fn density() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let first = attach(&mut driver, &device);
    let frame = scene();
    driver.draw(first, frame.clone()).unwrap();
    assert!(driver.poll(Instant::now()).is_empty());
    driver.close(first).unwrap();
    let output = graphics::shared::Output::new(frame.extent(), 2.0).unwrap();
    let target = driver.output(output).unwrap();
    driver
        .attach(Output {
            target,
            device: device.clone(),
        })
        .unwrap();
    driver.draw(target, frame.clone()).unwrap();
    device.0.borrow_mut().drawing = Ok(State::Complete);
    device.0.borrow_mut().presentation = State::Complete;
    assert!(driver.poll(Instant::now()).is_empty());
    assert!(driver.poll(Instant::now()).is_empty());
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].target, target);
    assert_eq!(receipts[0].frame, frame.id());
    assert!(receipts[0].result.is_ok());
    assert_eq!(device.0.borrow().copied, [frame.id()]);
    driver.close(target).unwrap();
}
