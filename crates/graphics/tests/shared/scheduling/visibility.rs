use super::*;

#[test]
fn dropped() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let target = attach(&mut driver, &device);
    let frame = scene();
    driver.draw(target, frame.clone()).unwrap();
    assert!(driver.poll(Instant::now()).is_empty());
    {
        let mut record = device.0.borrow_mut();
        record.drawing = Ok(State::Complete);
        record.presentation = State::Complete;
        record.visibility = Visibility::Dropped;
    }
    assert!(driver.poll(Instant::now()).is_empty());
    let completed = driver.poll(Instant::now());
    assert_eq!(completed.len(), 1);
    assert!(completed[0].result.is_ok());
    assert_eq!(completed[0].visible, None);
    assert_eq!(completed[0].drops, 1);
    assert!(driver.poll(Instant::now()).is_empty());
    assert_eq!(device.0.borrow().submitted, [frame.id()]);
    assert_eq!(device.0.borrow().copied, [frame.id(), frame.id()]);
    let visible = Instant::now();
    device.0.borrow_mut().visibility = Visibility::Visible(visible);
    assert!(driver.poll(Instant::now()).is_empty());
    let receipts = driver.visible();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].frame, frame.id());
    assert_eq!(receipts[0].visible, Some(visible));
    assert_eq!(receipts[0].drops, 1);
}

#[test]
fn deadline() {
    let device = Device::new();
    let mut driver = Driver::new(device.clone());
    let target = attach(&mut driver, &device);
    driver.draw(target, scene()).unwrap();
    let now = Instant::now();
    driver.poll(now);
    let receipts = driver.poll(now + Duration::from_secs(11));
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        receipts[0].result.as_ref().unwrap_err().0,
        "presentation deadline exceeded"
    );
    assert_eq!(Rc::strong_count(&device.0.borrow().resource), 2);
    device.0.borrow_mut().drawing = Ok(State::Complete);
    assert!(driver.poll(now + Duration::from_secs(12)).is_empty());
    assert_eq!(Rc::strong_count(&device.0.borrow().resource), 1);
    assert!(device.0.borrow().copied.is_empty());
    driver.close(target).unwrap();
}
