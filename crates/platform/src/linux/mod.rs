mod connection;
mod event;
mod input;
mod viewport;
mod window;

use crate::shared::{Console, Display, Event, Image, Input, Signal, Unit, Window as Options};
use connection::Connection;
use event::Wake;
use graphics::shared::Pacing;
use std::collections::BTreeMap;
use std::rc::Rc;
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::Mapping;

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

pub struct Desktop<E> {
    connection: Rc<Connection>,
    wake: Wake,
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
        Ok(Self {
            connection: Rc::new(Connection::open().map_err(convert)?),
            wake: Wake::new().map_err(convert)?,
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

    fn remap(&mut self, request: Mapping) -> Result<(), Error> {
        if request != Mapping::KEYBOARD {
            return Ok(());
        }
        let (first, columns, keysyms) = self.connection.keymap()?;
        for window in self.windows.values() {
            window.remap(first, columns, keysyms.clone());
        }
        Ok(())
    }

    fn pump(&mut self) -> Result<(), Error> {
        for _ in 0..256 {
            let Some(event) = self.connection.take()? else {
                break;
            };
            if let XEvent::MappingNotify(notify) = &event {
                self.remap(notify.request)?;
                continue;
            }
            let xid = match &event {
                XEvent::Expose(event) => event.window,
                XEvent::ConfigureNotify(event) => event.window,
                XEvent::ClientMessage(event) => event.window,
                XEvent::DestroyNotify(event) => event.window,
                XEvent::MapNotify(event) => event.window,
                XEvent::UnmapNotify(event) => event.window,
                XEvent::KeyPress(event) => event.event,
                XEvent::KeyRelease(event) => event.event,
                XEvent::ButtonPress(event) => event.event,
                XEvent::ButtonRelease(event) => event.event,
                XEvent::MotionNotify(event) => event.event,
                XEvent::LeaveNotify(event) => event.event,
                XEvent::FocusIn(event) => event.event,
                XEvent::FocusOut(event) => event.event,
                _ => continue,
            };
            let Some(unit) = self.xids.get(&xid).copied() else {
                continue;
            };
            if let Some(window) = self.windows.get_mut(&unit) {
                window.dispatch(&event)?;
            }
        }
        Ok(())
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
        self.window(unit)
            .and_then(|window| window.draw(frame))
            .map_err(self.convert)
    }

    fn notifier(&self) -> shared::Notify {
        self.wake.notifier()
    }

    fn wait(&mut self, pacing: Pacing) -> Result<(), E> {
        self.connection
            .check()
            .and_then(|()| self.connection.flush())
            .and_then(|()| {
                let buffered = self.connection.buffered()?;
                self.wake.wait(self.connection.fd(), buffered, pacing)
            })
            .map_err(self.convert)
    }

    fn close(&mut self, unit: Unit) -> Result<(), E> {
        self.connection.check().map_err(self.convert)?;
        let window = self
            .windows
            .remove(&unit)
            .ok_or_else(|| (self.convert)(Error("native window already closed".into())))?;
        self.xids.remove(&window.xid);
        Ok(())
    }
}

impl<E> Input<E> for Desktop<E> {
    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        self.connection
            .check()
            .and_then(|()| self.pump())
            .map_err(self.convert)?;
        for window in self.windows.values_mut() {
            window.refresh().map_err(self.convert)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Extent;

    #[test]
    fn refusal() {
        assert!(Desktop::<Error>::new("metal", |fault| fault).is_err());
        let mut desktop = match Desktop::new("cpu", |fault| fault) {
            Ok(desktop) => desktop,
            Err(_) => {
                eprintln!("skip linux::tests::refusal ownership: no reachable X11 display");
                return;
            }
        };
        let unit = Unit::new(1);
        let mut options = Options {
            title: "invalid\0title".into(),
            extent: Extent {
                width: 64,
                height: 32,
            },
        };
        assert!(desktop.create(unit, &options).is_err());
        options.title = "Plectron ownership probe".into();
        desktop.create(unit, &options).unwrap();
        assert!(desktop.create(unit, &options).is_err());
        desktop.close(unit).unwrap();
        assert!(desktop.title(unit, "closed").is_err());
        assert!(desktop.close(unit).is_err());
    }
}
