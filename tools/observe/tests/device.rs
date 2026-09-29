use plectron::contract::{Fault, Unit, Window};
use plectron::platform::device::Signal;
use plectron::platform::{Console, Display, Input};
use plectron::rendering::Image;
use std::cell::Cell;
use std::rc::Rc;

pub struct Device(pub Rc<Cell<usize>>);
impl Display<Fault> for Device {
    fn create(&mut self, _: Unit, _: &Window) -> Result<(), Fault> {
        Err(Fault("probe does not create windows".into()))
    }
    fn title(&mut self, _: Unit, _: &str) -> Result<(), Fault> {
        unreachable!()
    }
    fn present(&mut self, _: Unit, _: Image) -> Result<(), Fault> {
        unreachable!()
    }
    fn close(&mut self, _: Unit) -> Result<(), Fault> {
        unreachable!()
    }
}
impl Input<Fault> for Device {
    fn poll(&mut self) -> Result<Vec<Signal>, Fault> {
        Ok(Vec::new())
    }
}
impl Console<Fault> for Device {
    fn log(&mut self, text: &str) -> Result<(), Fault> {
        assert_eq!(text, "1");
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
