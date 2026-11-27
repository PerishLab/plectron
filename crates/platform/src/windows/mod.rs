mod class;
mod event;
pub mod fonts;
mod graphics;
mod input;
mod taskbar;
mod timing;
mod viewport;
mod window;
#[cfg(test)]
use crate::shared::Input;
use crate::shared::{
    Console, Dialog, Display, Image, Outcome, Progress, Receipt, Region, Unit, Visibility,
    Window as Options,
};
use ::graphics::shared::Pacing;
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
    graphics: graphics::Graphics,
    timing: timing::Timing,
    demand: BTreeMap<Unit, Demand>,
    windows: BTreeMap<Unit, window::Window>,
    convert: fn(Error) -> E,
}
impl<E> Desktop<E> {
    pub fn new(name: &str, convert: fn(Error) -> E) -> Result<Self, E> {
        let class = Rc::new(class::Class::new().map_err(convert)?);
        let taskbar = Rc::new(taskbar::Taskbar::new());
        let events = event::Events::new().map_err(convert)?;
        let graphics = graphics::Graphics::new(name, events.notifier()).map_err(convert)?;
        let timing = timing::Timing::new(events.notifier());
        Ok(Self {
            class,
            taskbar,
            events,
            graphics,
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
        pacing || self.graphics.awaiting() || self.windows.values().any(window::Window::awaiting)
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
    fn prepare(&mut self) -> Result<(), E> {
        self.graphics.prepare().map_err(self.convert)
    }
    fn create(&mut self, unit: Unit, options: &Options) -> Result<(), E> {
        if self.windows.contains_key(&unit) {
            return Err((self.convert)(Error(
                "native window identity already exists".into(),
            )));
        }
        let window = window::Window::new(self.class.clone(), self.taskbar.clone(), options)
            .map_err(self.convert)?;
        let output = graphics::Graphics::output(
            &window,
            ::graphics::shared::Extent {
                width: options.extent.width,
                height: options.extent.height,
            },
        )
        .map_err(self.convert)?;
        self.graphics
            .create(unit, &window, output)
            .map_err(self.convert)?;
        window.accelerate(self.graphics.active());
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
    fn dialog(&mut self, unit: Unit, request: &Dialog) -> Result<Outcome, E> {
        self.window(unit)
            .and_then(|window| window.dialog(request))
            .map_err(self.convert)
    }
    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), E> {
        let window = self.windows.get(&unit).ok_or_else(|| {
            (self.convert)(Error(format!("native window {} is absent", unit.id())))
        })?;
        if window.accelerated() {
            return self
                .graphics
                .image(unit, window, &frame)
                .map_err(self.convert);
        }
        window.present(frame).map_err(self.convert)
    }
    fn draw(
        &mut self,
        unit: Unit,
        frame: &::graphics::shared::Frame,
    ) -> Result<::graphics::shared::State, E> {
        let window = self.windows.get(&unit).ok_or_else(|| {
            (self.convert)(Error(format!("native window {} is absent", unit.id())))
        })?;
        if window.accelerated() {
            self.graphics
                .draw(unit, window, frame)
                .map_err(self.convert)?;
            return Ok(::graphics::shared::State::Pending);
        }
        let compute = self.graphics.compute().map_err(self.convert)?;
        window.draw(frame, compute).map_err(self.convert)
    }
    fn completed(&mut self) -> Vec<Receipt<E>> {
        self.graphics.poll();
        let mut receipts = Vec::new();
        for recovery in self.graphics.recoveries() {
            receipts.push(self.recover(recovery));
        }
        for receipt in self.graphics.completed() {
            if receipt.result.is_ok()
                && let Some(window) = self.windows.get(&receipt.unit)
            {
                let _ = window.expose();
            }
            receipts.push(receipt);
        }
        if let Ok(compute) = self.graphics.compute() {
            for (unit, window) in &self.windows {
                if !window.accelerated() {
                    receipts.extend(window.completed(*unit, compute, &self.timing));
                }
            }
        }
        self.timing.engage(self.timing.native() && self.engaged());
        receipts
            .into_iter()
            .map(|receipt| receipt.map(self.convert))
            .collect()
    }
    fn visible(&mut self) -> Vec<Visibility> {
        self.graphics.poll();
        self.settle();
        let mut seen = self.graphics.visible();
        for (unit, window) in &self.windows {
            if !window.accelerated() {
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
        }
        self.timing.engage(self.timing.native() && self.engaged());
        seen
    }
    fn pulse(&mut self, unit: Unit) -> Result<Option<::graphics::shared::Pulse>, E> {
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
        } else if self.graphics.awaiting() {
            Pacing::At(Instant::now() + Duration::from_millis(16))
        } else {
            pacing
        };
        self.events.wait(pacing).map_err(self.convert)
    }
    fn close(&mut self, unit: Unit) -> Result<(), E> {
        self.class.check().map_err(self.convert)?;
        if !self.windows.contains_key(&unit) {
            return Err((self.convert)(Error("native window already closed".into())));
        }
        self.graphics.close(unit).map_err(self.convert)?;
        self.windows.remove(&unit);
        self.demand.remove(&unit);
        self.timing.engage(self.timing.native() && self.engaged());
        Ok(())
    }
}
impl<E> Console<E> for Desktop<E> {
    fn log(&mut self, text: &str) -> Result<(), E> {
        println!("{text}");
        Ok(())
    }
}
