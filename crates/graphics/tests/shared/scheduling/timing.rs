use super::*;
use std::cell::Cell;

struct Clock {
    target: Target,
    calls: Rc<Cell<usize>>,
}
impl Sink<(u64, Rc<()>)> for Clock {
    fn target(&self) -> Target {
        self.target
    }
    fn present(&mut self, _: &(u64, Rc<()>)) -> Result<(), Fault> {
        Err(Fault("presentation admission refused".into()))
    }
    fn completed(&self) -> Option<Instant> {
        self.calls.set(self.calls.get() + 1);
        Some(Instant::now())
    }
}
#[test]
fn admission() {
    let device = Device::new();
    device.0.borrow_mut().drawing = Ok(State::Complete);
    let mut driver = Driver::new(device);
    let target = driver.target(scene().extent()).unwrap();
    let calls = Rc::new(Cell::new(0));
    driver
        .attach(Clock {
            target,
            calls: calls.clone(),
        })
        .unwrap();
    driver.draw(target, scene()).unwrap();
    assert!(driver.poll(Instant::now()).is_empty());
    let receipts = driver.poll(Instant::now());
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].result.is_err());
    assert_eq!(receipts[0].produced, None);
    assert_eq!(receipts[0].visible, None);
    assert_eq!(calls.get(), 0);
}
