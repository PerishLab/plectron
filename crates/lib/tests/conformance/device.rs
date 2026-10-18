use plectron::contract::{Fault, Unit, Window};
use plectron::platform::device::{Button, Event, Phase, Point, Signal};
use plectron::platform::{Console, Display, Input, Platform};
use plectron::rendering::{Image, Solid};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Default)]
pub struct Record {
    pub windows: BTreeMap<Unit, Window>,
    pub titles: Vec<String>,
    pub frames: Vec<Image>,
    pub signals: Vec<Signal>,
    pub logs: Vec<String>,
    pub closed: Vec<Unit>,
    pub fail: bool,
    pub reject: Option<&'static str>,
    pub deferred: bool,
    pub submissions: Vec<(Unit, graphics::shared::Frame)>,
    pub receipts: Vec<plectron::platform::Receipt>,
    pub focus: Vec<(Unit, Option<::platform::shared::Focus>)>,
    pub prepared: usize,
    pub activated: Vec<Unit>,
}

#[derive(Clone, Default)]
pub struct Device(pub Rc<RefCell<Record>>);

pub fn setup() -> (Platform<Device, Solid>, Device) {
    let device = Device::default();
    (Platform::new(device.clone(), Solid), device)
}

pub fn click(unit: Unit) -> [Signal; 2] {
    tap(unit, None)
}

pub fn tap(unit: Unit, position: Option<Point>) -> [Signal; 2] {
    [Phase::Down(Button::Primary), Phase::Up(Button::Primary)].map(|phase| Signal {
        unit,
        event: Event::Pointer { position, phase },
    })
}

pub fn at(x: usize, y: usize) -> Point {
    Point {
        x: x as i64,
        y: y as i64,
    }
}

pub fn close(unit: Unit) -> Signal {
    Signal {
        unit,
        event: Event::Close,
    }
}

impl Display<Fault> for Device {
    fn prepare(&mut self) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        record.prepared += 1;
        if record.reject == Some("prepare") {
            return Err(Fault("preparation rejected".into()));
        }
        Ok(())
    }

    fn create(&mut self, unit: Unit, options: &Window) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        if record.fail {
            return Err(Fault("creation rejected".into()));
        }
        record.windows.insert(unit, options.clone());
        Ok(())
    }
    fn activate(&mut self, unit: Unit) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        assert!(record.windows.contains_key(&unit));
        if record.reject == Some("activate") {
            return Err(Fault("activation rejected".into()));
        }
        if !record.activated.contains(&unit) {
            record.activated.push(unit);
        }
        Ok(())
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        assert!(record.windows.contains_key(&unit));
        record.titles.push(title.into());
        Ok(())
    }
    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        assert!(record.windows.contains_key(&unit));
        record.frames.push(frame);
        Ok(())
    }
    fn draw(
        &mut self,
        unit: Unit,
        scene: &graphics::shared::Frame,
    ) -> Result<graphics::shared::State, Fault> {
        if self.0.borrow().deferred {
            self.0.borrow_mut().submissions.push((unit, scene.clone()));
            return Ok(graphics::shared::State::Pending);
        }
        let image = graphics::shared::Backend::render(&mut graphics::shared::Cpu, scene).unwrap();
        self.present(
            unit,
            Image {
                extent: plectron::contract::Extent {
                    width: image.extent.width,
                    height: image.extent.height,
                },
                pixels: image.pixels,
            },
        )?;
        Ok(graphics::shared::State::Complete)
    }
    fn completed(&mut self) -> Vec<plectron::platform::Receipt> {
        std::mem::take(&mut self.0.borrow_mut().receipts)
    }
    fn close(&mut self, unit: Unit) -> Result<(), Fault> {
        let mut record = self.0.borrow_mut();
        assert!(record.windows.remove(&unit).is_some());
        record.closed.push(unit);
        Ok(())
    }
}

impl Input<Fault> for Device {
    fn focus(
        &mut self,
        unit: Unit,
        context: Option<&::platform::shared::Focus>,
    ) -> Result<(), Fault> {
        self.0.borrow_mut().focus.push((unit, context.cloned()));
        Ok(())
    }

    fn poll(&mut self) -> Result<Vec<Signal>, Fault> {
        Ok(std::mem::take(&mut self.0.borrow_mut().signals))
    }
}

impl Console<Fault> for Device {
    fn log(&mut self, text: &str) -> Result<(), Fault> {
        self.0.borrow_mut().logs.push(text.into());
        Ok(())
    }
}

pub fn runtime() -> (
    plectron::runtime::Runtime<plectron::addons::control::Control<Platform<Device, Solid>>>,
    Device,
) {
    let (host, device) = setup();
    let runtime =
        plectron::runtime::Runtime::new(plectron::addons::control::Control::new(host)).unwrap();
    (runtime, device)
}

pub fn control() -> (
    plectron::runtime::Runtime<plectron::addons::control::Control<Platform<Device, Solid>>>,
    Device,
) {
    let (mut runtime, device) = runtime();
    runtime
        .execute(&fixtures::script("platform, console, surface", ""))
        .unwrap();
    (runtime, device)
}
