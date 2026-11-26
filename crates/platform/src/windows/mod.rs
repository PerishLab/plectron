mod class;
mod event;
pub mod fonts;
mod input;
mod taskbar;
mod timing;
mod viewport;
mod window;
use crate::shared::{
    Console, Display, Event, Image, Input, Progress, Receipt, Region, Signal, Unit, Visibility,
    Window as Options,
};
use graphics::shared::{Compute, Pacing};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

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
    class: Rc<class::Class>,
    taskbar: Rc<taskbar::Taskbar>,
    events: event::Events,
    compute: Compute,
    timing: timing::Timing,
    demand: BTreeMap<Unit, Demand>,
    windows: BTreeMap<Unit, window::Window>,
    convert: fn(Error) -> E,
}
impl<E> Desktop<E> {
    pub fn new(name: &str, convert: fn(Error) -> E) -> Result<Self, E> {
        if name != "cpu" {
            return Err(convert(Error(
                "Windows desktop currently requires the CPU renderer".into(),
            )));
        }
        let class = Rc::new(class::Class::new().map_err(convert)?);
        let taskbar = Rc::new(taskbar::Taskbar::new());
        let events = event::Events::new().map_err(convert)?;
        let compute = Compute::new(events.notifier())
            .map_err(error)
            .map_err(convert)?;
        let timing = timing::Timing::new(events.notifier());
        Ok(Self {
            class,
            taskbar,
            events,
            compute,
            timing,
            demand: BTreeMap::new(),
            windows: BTreeMap::new(),
            convert,
        })
    }
    fn window(&self, unit: Unit) -> Result<&window::Window, Error> {
        self.windows
            .get(&unit)
            .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))
    }
    fn engaged(&self) -> bool {
        let pacing = self
            .demand
            .values()
            .any(|demand| timing::due(demand.pacing));
        pacing || self.windows.values().any(window::Window::awaiting)
    }
    fn settle(&self) {
        if let Some(sample) = self.timing.latest() {
            for window in self.windows.values() {
                window.sight(sample);
            }
        }
    }
}
impl<E> Display<E> for Desktop<E> {
    fn create(&mut self, unit: Unit, options: &Options) -> Result<(), E> {
        if self.windows.contains_key(&unit) {
            return Err((self.convert)(Error(
                "native window identity already exists".into(),
            )));
        }
        let window = window::Window::new(self.class.clone(), self.taskbar.clone(), options)
            .map_err(self.convert)?;
        self.windows.insert(unit, window);
        Ok(())
    }
    fn activate(&mut self, unit: Unit) -> Result<(), E> {
        self.window(unit)
            .and_then(window::Window::activate)
            .map_err(self.convert)
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.title(title))
            .map_err(self.convert)
    }
    fn drag(&mut self, unit: Unit, regions: &[Region]) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.drag(regions))
            .map_err(self.convert)
    }
    fn progress(&mut self, unit: Unit, progress: Progress) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.progress(progress))
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
            .and_then(|window| window.draw(frame, &self.compute))
            .map_err(self.convert)
    }
    fn completed(&mut self) -> Vec<Receipt<E>> {
        let mut receipts = Vec::new();
        for (unit, window) in &self.windows {
            receipts.extend(window.completed(*unit, &self.compute, &self.timing));
        }
        self.timing.engage(self.timing.native() && self.engaged());
        receipts
            .into_iter()
            .map(|receipt| receipt.map(self.convert))
            .collect()
    }
    fn visible(&mut self) -> Vec<Visibility> {
        self.settle();
        self.timing.engage(self.timing.native() && self.engaged());
        let mut seen = Vec::new();
        for (unit, window) in &self.windows {
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
        self.window(unit).map_err(self.convert)?;
        let demand = self.demand.entry(unit).or_insert(IDLE);
        Ok(self.timing.pulse(&mut demand.seen, demand.pacing))
    }
    fn pace(&mut self, unit: Unit, pacing: Pacing) -> Result<(), E> {
        self.window(unit).map_err(self.convert)?;
        self.demand.entry(unit).or_insert(IDLE).pacing = pacing;
        self.timing.engage(self.timing.native() && self.engaged());
        Ok(())
    }
    fn notifier(&self) -> shared::Notify {
        self.events.notifier()
    }
    fn wait(&mut self, pacing: Pacing) -> Result<(), E> {
        self.class.check().map_err(self.convert)?;
        let active = self.timing.native() && self.engaged();
        self.timing.engage(active);
        let pacing = if active {
            Pacing::At(Instant::now() + Duration::from_millis(250))
        } else {
            pacing
        };
        self.events.wait(pacing).map_err(self.convert)
    }
    fn close(&mut self, unit: Unit) -> Result<(), E> {
        self.class.check().map_err(self.convert)?;
        self.windows
            .remove(&unit)
            .ok_or_else(|| (self.convert)(Error("native window already closed".into())))?;
        self.demand.remove(&unit);
        self.timing.engage(self.timing.native() && self.engaged());
        Ok(())
    }
}
impl<E> Input<E> for Desktop<E> {
    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        self.class
            .check()
            .and_then(|()| self.events.poll())
            .map_err(self.convert)?;
        self.settle();
        for window in self.windows.values() {
            window
                .result()
                .and_then(|()| window.refresh(&self.compute))
                .map_err(self.convert)?;
        }
        let mut signals = Vec::new();
        for (unit, window) in &self.windows {
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
                    self.events.notifier().notify();
                }
            }
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
mod cadence;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Extent;

    #[test]
    fn refusal() {
        assert!(Desktop::<Error>::new("metal", |fault| fault).is_err());
        let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
        let unit = Unit::new(1);
        let mut options = Options::new(
            "invalid\0title",
            Extent {
                width: 64,
                height: 32,
            },
        );
        assert!(desktop.create(unit, &options).is_err());
        options.title = "Plectron ownership probe".into();
        desktop.create(unit, &options).unwrap();
        assert!(desktop.create(unit, &options).is_err());
        desktop.close(unit).unwrap();
        assert!(desktop.title(unit, "closed").is_err());
        assert!(desktop.close(unit).is_err());
    }
}
