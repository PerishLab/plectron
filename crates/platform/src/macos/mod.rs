mod application;
mod event;
mod events;
mod graphics;
mod ime;
mod input;
mod timing;
mod window;

use self::graphics::Graphics;
use crate::shared::{
    Console, Display, Focus, Image, Input, Receipt, Signal, Unit, Visibility, Window as Options,
};
use std::collections::BTreeMap;
use window::Window;

pub use ime::Text;
pub use input::Keyboard;

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Error {}

pub(super) fn error(value: impl std::fmt::Display) -> Error {
    Error(value.to_string())
}

pub struct Desktop<E> {
    application: application::Application,
    windows: BTreeMap<Unit, Canvas>,
    graphics: Graphics,
    events: event::Events,
    receipts: Vec<Receipt<Error>>,
    convert: fn(Error) -> E,
}

struct Canvas {
    output: ::graphics::shared::Output,
    scene: Option<::graphics::shared::Frame>,
    ime: ime::View,
    timing: timing::Timing,
    window: Window,
    frame: Image,
    dirty: bool,
    input: Keyboard,
    pointer: events::Pointer,
    pending: Option<u64>,
    task: Option<(u64, std::time::Instant, ::graphics::shared::Task)>,
    active: bool,
    accelerated: bool,
}

impl<E> Desktop<E> {
    pub fn new(name: &str, convert: fn(Error) -> E) -> Result<Self, E> {
        let application = application::Application::new().map_err(convert)?;
        let events = event::Events::new().map_err(convert)?;
        let graphics = Graphics::new(name, events.notifier()).map_err(convert)?;
        Ok(Self {
            application,
            windows: BTreeMap::new(),
            graphics,
            events,
            receipts: Vec::new(),
            convert,
        })
    }

    fn canvas(&mut self, unit: Unit) -> Result<&mut Canvas, Error> {
        self.windows
            .get_mut(&unit)
            .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))
    }
}

impl<E> Display<E> for Desktop<E> {
    fn prepare(&mut self) -> Result<(), E> {
        self.graphics.prepare().map_err(self.convert)
    }

    fn create(&mut self, unit: Unit, options: &Options) -> Result<(), E> {
        (|| {
            let extent = options.extent;
            let pixels = vec![0x182330; extent.area().map_err(error)?];
            let window = Window::new(&options.title, extent.width, extent.height)?;
            let input = Keyboard::default();
            let timing = timing::Timing::new(&window)?;
            let output = Graphics::output(
                &window,
                ::graphics::shared::Extent {
                    width: extent.width,
                    height: extent.height,
                },
            )?;
            self.graphics.create(unit, &window, output)?;
            let accelerated = self.graphics.active();
            window.show();
            self.application.activate();
            window.key();
            let ime = ime::View::new(&window)?;
            self.windows.insert(
                unit,
                Canvas {
                    output,
                    scene: None,
                    ime,
                    timing,
                    window,
                    frame: Image { extent, pixels },
                    dirty: true,
                    input,
                    pointer: events::Pointer::default(),
                    pending: None,
                    task: None,
                    active: true,
                    accelerated,
                },
            );
            eprintln!(
                "[window:{}] created {:?} {}x{}",
                unit.id(),
                options.title,
                extent.width,
                extent.height
            );
            Ok(())
        })()
        .map_err(self.convert)
    }

    fn activate(&mut self, unit: Unit) -> Result<(), E> {
        (|| {
            let canvas = self
                .windows
                .get(&unit)
                .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))?;
            if canvas.active {
                return Ok(());
            }
            self.graphics.create(unit, &canvas.window, canvas.output)?;
            let canvas = self.windows.get_mut(&unit).expect("checked canvas");
            canvas.accelerated = self.graphics.active();
            canvas.window.show();
            self.application.activate();
            canvas.window.key();
            canvas.active = true;
            Ok(())
        })()
        .map_err(self.convert)
    }

    fn title(&mut self, unit: Unit, title: &str) -> Result<(), E> {
        (|| {
            self.canvas(unit)?.window.title(title)?;
            eprintln!("[window:{}] title {:?}", unit.id(), title);
            Ok(())
        })()
        .map_err(self.convert)
    }

    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), E> {
        (|| {
            if self.canvas(unit)?.accelerated {
                self.graphics.image(unit, &frame)?;
                self.canvas(unit)?.frame = frame;
            } else {
                let canvas = self
                    .windows
                    .get_mut(&unit)
                    .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))?;
                canvas.image(frame, &self.graphics)?;
            }
            Ok(())
        })()
        .map_err(self.convert)
    }

    fn draw(
        &mut self,
        unit: Unit,
        frame: &::graphics::shared::Frame,
    ) -> Result<::graphics::shared::State, E> {
        (|| {
            if self.canvas(unit)?.accelerated {
                self.graphics.draw(unit, frame)?;
            } else {
                let canvas = self
                    .windows
                    .get_mut(&unit)
                    .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))?;
                canvas.draw(frame, &self.graphics)?;
            }
            self.canvas(unit)?.scene = Some(frame.clone());
            Ok(::graphics::shared::State::Pending)
        })()
        .map_err(self.convert)
    }

    fn completed(&mut self) -> Vec<Receipt<E>> {
        self.graphics.poll();
        for recovery in self.graphics.recoveries() {
            let receipt = self.recover(recovery);
            self.receipts.push(receipt);
        }
        self.receipts.extend(self.graphics.completed());
        std::mem::take(&mut self.receipts)
            .into_iter()
            .map(|receipt| receipt.map(self.convert))
            .collect()
    }

    fn visible(&mut self) -> Vec<Visibility> {
        self.graphics.visible()
    }

    fn pulse(&mut self, unit: Unit) -> Result<Option<::graphics::shared::Pulse>, E> {
        self.canvas(unit)
            .map(|canvas| canvas.timing.pulse())
            .map_err(self.convert)
    }

    fn pace(&mut self, unit: Unit, pacing: ::graphics::shared::Pacing) -> Result<(), E> {
        self.canvas(unit)
            .map(|canvas| canvas.timing.pace(pacing))
            .map_err(self.convert)
    }

    fn notifier(&self) -> shared::Notify {
        self.events.notifier()
    }

    fn wait(&mut self, pacing: ::graphics::shared::Pacing) -> Result<(), E> {
        let fallback = self.windows.values().any(|canvas| !canvas.timing.native());
        let native = self.windows.values().any(|canvas| canvas.timing.active());
        let pacing = if native {
            ::graphics::shared::Pacing::Continuous
        } else if fallback && pacing == ::graphics::shared::Pacing::Continuous {
            ::graphics::shared::Pacing::At(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            )
        } else {
            pacing
        };
        self.events.wait(pacing);
        Ok(())
    }

    fn close(&mut self, unit: Unit) -> Result<(), E> {
        (|| {
            if self.canvas(unit)?.accelerated {
                self.graphics.close(unit)?;
            }
            self.receipts.retain(|receipt| receipt.unit != unit);
            self.windows
                .remove(&unit)
                .ok_or_else(|| Error("native window already closed".into()))?;
            eprintln!("[window:{}] closed", unit.id());
            Ok(())
        })()
        .map_err(self.convert)
    }
}

impl<E> Input<E> for Desktop<E> {
    fn focus(&mut self, unit: Unit, context: Option<&Focus>) -> Result<(), E> {
        self.canvas(unit)
            .and_then(|canvas| canvas.ime.context(context))
            .map_err(self.convert)
    }

    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        (|| {
            self.application.poll();
            let mut signals = Vec::new();
            for (unit, canvas) in &mut self.windows {
                canvas.refresh(*unit, &mut self.graphics)?;
                canvas.advance(
                    (*unit, canvas.accelerated),
                    &mut signals,
                    &mut self.receipts,
                )?;
            }
            Ok(signals)
        })()
        .map_err(self.convert)
    }
}

impl<E> Console<E> for Desktop<E> {
    fn log(&mut self, text: &str) -> Result<(), E> {
        println!("{text}");
        Ok(())
    }
}
