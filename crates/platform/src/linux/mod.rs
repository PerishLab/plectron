mod connection;
mod event;
mod input;
mod paint;
mod pump;
mod timing;
mod viewport;
mod window;

use crate::shared::{
    Console, Display, Event, Focus, Image, Input, Receipt, Signal, Unit, Visibility,
    Window as Options,
};
use connection::Connection;
use event::Wake;
use graphics::shared::{Compute, Pacing};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for Error {}
fn error(value: impl std::fmt::Display) -> Error {
    Error(value.to_string())
}

struct Demand {
    seen: u64,
    pacing: Pacing,
}

const IDLE: Demand = Demand {
    seen: 0,
    pacing: Pacing::Idle,
};

pub struct Desktop<E> {
    connection: Rc<Connection>,
    wake: Wake,
    compute: Compute,
    demand: BTreeMap<Unit, Demand>,
    windows: BTreeMap<Unit, window::Window>,
    xids: BTreeMap<u32, Unit>,
    convert: fn(Error) -> E,
}

impl<E> Desktop<E> {
    pub fn new(name: &str, convert: fn(Error) -> E) -> Result<Self, E> {
        if name != "cpu" {
            return Err(convert(Error(
                "Linux desktop currently requires the CPU renderer".into(),
            )));
        }
        let connection = Rc::new(Connection::open().map_err(convert)?);
        let wake = Wake::new().map_err(convert)?;
        let compute = Compute::new(wake.notifier())
            .map_err(error)
            .map_err(convert)?;
        Ok(Self {
            connection,
            wake,
            compute,
            demand: BTreeMap::new(),
            windows: BTreeMap::new(),
            xids: BTreeMap::new(),
            convert,
        })
    }

    fn window(&mut self, unit: Unit) -> Result<&mut window::Window, Error> {
        self.windows
            .get_mut(&unit)
            .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))
    }

    fn request(&self) -> Result<bool, Error> {
        let due = self.windows.iter().filter_map(|(unit, window)| {
            let demand = self.demand.get(unit)?;
            timing::due(demand.pacing).then_some(&window.timing)
        });
        timing::request(&self.connection, due)
    }
}

impl<E> Display<E> for Desktop<E> {
    fn create(&mut self, unit: Unit, options: &Options) -> Result<(), E> {
        if self.windows.contains_key(&unit) {
            return Err((self.convert)(Error(
                "native window identity already exists".into(),
            )));
        }
        let window = window::Window::new(self.connection.clone(), options).map_err(self.convert)?;
        self.xids.insert(window.xid, unit);
        self.windows.insert(unit, window);
        Ok(())
    }

    fn activate(&mut self, unit: Unit) -> Result<(), E> {
        self.window(unit)
            .and_then(window::Window::show)
            .map_err(self.convert)
    }

    fn title(&mut self, unit: Unit, title: &str) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.title(title))
            .map_err(self.convert)
    }

    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.present(frame))
            .map_err(self.convert)
    }

    fn draw(
        &mut self,
        unit: Unit,
        frame: &graphics::shared::Frame,
    ) -> Result<graphics::shared::State, E> {
        let compute = &self.compute;
        self.windows
            .get_mut(&unit)
            .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))
            .and_then(|window| window.draw(frame, compute))
            .map_err(self.convert)
    }

    fn completed(&mut self) -> Vec<Receipt<E>> {
        let mut receipts = Vec::new();
        for (unit, window) in &mut self.windows {
            receipts.extend(window.completed(*unit, &self.compute));
        }
        receipts
            .into_iter()
            .map(|receipt| receipt.map(self.convert))
            .collect()
    }

    fn visible(&mut self) -> Vec<Visibility> {
        let mut seen = Vec::new();
        for (unit, window) in &mut self.windows {
            seen.extend(
                window
                    .sighted()
                    .into_iter()
                    .map(|(frame, visible)| Visibility {
                        unit: *unit,
                        frame,
                        visible,
                        drops: 0,
                    }),
            );
        }
        seen
    }

    fn pulse(&mut self, unit: Unit) -> Result<Option<graphics::shared::Pulse>, E> {
        let convert = self.convert;
        self.window(unit).map_err(convert)?;
        let demand = self.demand.entry(unit).or_insert(IDLE);
        Ok(self.windows[&unit]
            .timing
            .pulse(&mut demand.seen, demand.pacing))
    }

    fn pace(&mut self, unit: Unit, pacing: Pacing) -> Result<(), E> {
        let convert = self.convert;
        self.window(unit).map_err(convert)?;
        self.demand.entry(unit).or_insert(IDLE).pacing = pacing;
        Ok(())
    }

    fn notifier(&self) -> shared::Notify {
        self.wake.notifier()
    }

    fn wait(&mut self, pacing: Pacing) -> Result<(), E> {
        let wait = || {
            let native = self.request()?;
            self.connection.flush()?;
            let pacing = if native {
                Pacing::At(std::time::Instant::now() + std::time::Duration::from_millis(250))
            } else {
                pacing
            };
            let buffered = self.connection.buffered()?;
            self.wake.wait(self.connection.fd(), buffered, pacing)
        };
        wait().map_err(self.convert)
    }

    fn close(&mut self, unit: Unit) -> Result<(), E> {
        let window = self
            .windows
            .remove(&unit)
            .ok_or_else(|| (self.convert)(Error("native window already closed".into())))?;
        self.xids.remove(&window.xid);
        self.demand.remove(&unit);
        Ok(())
    }
}

impl<E> Input<E> for Desktop<E> {
    fn focus(&mut self, unit: Unit, context: Option<&Focus>) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.focus(context.map(|focus| focus.target)))
            .map_err(self.convert)
    }

    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        self.pump().map_err(self.convert)?;
        for window in self.windows.values_mut() {
            window.refresh(&self.compute).map_err(self.convert)?;
        }
        let mut signals = Vec::new();
        let mut notify = false;
        for (unit, window) in &mut self.windows {
            if window.closing() {
                signals.push(Signal {
                    unit: *unit,
                    event: Event::Close,
                });
            } else {
                signals.extend(
                    window
                        .input()
                        .into_iter()
                        .map(|event| Signal { unit: *unit, event }),
                );
                if window.pending() {
                    notify = true;
                }
            }
        }
        if notify {
            self.wake.notifier().notify();
        }
        Ok(signals)
    }
}

impl<E> Console<E> for Desktop<E> {
    fn log(&mut self, text: &str) -> Result<(), E> {
        println!("{text}");
        Ok(())
    }
}

#[path = "../../tests/linux/cadence.rs"]
mod cadence;

#[path = "../../tests/linux/thread.rs"]
mod thread;
