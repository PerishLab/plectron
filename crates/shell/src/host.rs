use crate::bundle::Bundle;
use crate::convert::{compose, coordinates, focus};
use crate::memory::Memory;
use platform::shared::{
    Backend, Button, Chrome, Event, Extent, Phase, Point, Signal, Unit, Window,
};
use std::sync::Arc;
use webengine::{Action, Fault, Hit, State, Worker};

pub struct Options {
    pub title: String,
    pub extent: Extent,
    pub renderer: String,
    pub chrome: Chrome,
}

impl Options {
    pub fn new(title: impl Into<String>, width: usize, height: usize) -> Self {
        Self {
            title: title.into(),
            extent: Extent { width, height },
            renderer: "cpu".into(),
            chrome: Chrome::Native,
        }
    }
}

pub struct Runner<B> {
    backend: B,
    unit: Unit,
    extent: Extent,
    worker: Worker,
    pointer: Option<(usize, usize)>,
    hovered: Option<Hit>,
    pressed: Option<Hit>,
    closed: bool,
}

impl<B: Backend<Fault>> Runner<B> {
    pub fn open(mut backend: B, bundle: Bundle, options: Options) -> Result<Self, Fault> {
        options
            .extent
            .area()
            .map_err(|error| Fault(error.to_string()))?;
        let unit = Unit::new(1);
        let notify = backend.notifier();
        let memory = Memory::new(Arc::new(bundle)).system();
        let entry = memory.entry().to_owned();
        backend.prepare()?;
        let mut window = Window::new(options.title, options.extent);
        window.chrome = options.chrome;
        backend.create(unit, &window)?;
        backend.activate(unit)?;
        let worker = Worker::notified(
            &entry,
            domengine::Extent {
                width: options.extent.width,
                height: options.extent.height,
            },
            memory,
            (config::Store::default(), Some(notify)),
        )?;
        Ok(Self {
            backend,
            unit,
            extent: options.extent,
            worker,
            pointer: None,
            hovered: None,
            pressed: None,
            closed: false,
        })
    }

    pub fn active(&self) -> bool {
        !self.closed
    }

    pub fn tick(&mut self) -> Result<(), Fault> {
        if self.closed {
            return Ok(());
        }
        for signal in self.backend.poll()? {
            self.signal(signal)?;
            if self.closed {
                return Ok(());
            }
        }
        for receipt in self.backend.completed() {
            if receipt.unit == self.unit {
                let _ = self.worker.complete(receipt.frame, receipt.result)?;
                self.hover()?;
            }
        }
        if let Some(pulse) = self.backend.pulse(self.unit)? {
            self.worker.pulse(pulse)?;
        }
        self.worker.poll()?;
        self.backend.pace(self.unit, self.worker.pacing()?)?;
        self.submit()
    }

    pub fn wait(&mut self) -> Result<(), Fault> {
        if self.closed {
            return Ok(());
        }
        self.backend.wait(self.worker.pacing()?)
    }

    pub fn drive(&mut self) -> Result<(), Fault> {
        while self.active() {
            self.tick()?;
            if self.active() {
                self.wait()?;
            }
        }
        Ok(())
    }

    fn signal(&mut self, signal: Signal) -> Result<(), Fault> {
        if signal.unit != self.unit {
            return Ok(());
        }
        match signal.event {
            Event::Close => {
                self.closed = true;
                self.worker.close();
                self.backend.close(self.unit)?;
            }
            Event::Viewport {
                width,
                height,
                density,
            } => self.resize((width, height), density)?,
            Event::Scroll { delta } => {
                self.worker.scroll(delta)?;
                self.submit()?;
            }
            Event::Wheel { position, delta } => {
                if let Some((x, y)) = coordinates(position)
                    && self.worker.wheel(x, y, delta)?.applied()
                {
                    self.submit()?;
                }
            }
            Event::Pointer { position, phase } => self.pointer(position, phase)?,
            Event::Input(input) => {
                if self.worker.state() == State::Ready {
                    let _ = self.worker.submit(Action::Input(input))?;
                }
            }
            Event::Composition(input) => {
                if self.worker.state() == State::Ready {
                    let _ = self.worker.submit(Action::Composition(compose(input)))?;
                }
            }
        }
        Ok(())
    }

    fn resize(&mut self, size: (usize, usize), density: (u32, u32)) -> Result<(), Fault> {
        if density.1 == 0 {
            return Err(Fault("viewport density denominator is zero".into()));
        }
        let extent = domengine::Extent {
            width: size.0,
            height: size.1,
        };
        graphics::shared::Output::new(extent, f64::from(density.0) / f64::from(density.1))
            .map_err(|fault| Fault(fault.to_string()))?;
        self.extent = Extent {
            width: size.0,
            height: size.1,
        };
        self.pointer = None;
        self.hovered = None;
        self.pressed = None;
        let _ = self.worker.submit(Action::Resize(extent))?;
        Ok(())
    }

    fn pointer(&mut self, position: Option<Point>, phase: Phase) -> Result<(), Fault> {
        let point = position.and_then(coordinates);
        match phase {
            Phase::Move => {
                self.pointer = point;
                self.hover()
            }
            Phase::Down(Button::Primary) => {
                self.pressed = point.map(|(x, y)| self.worker.hit(x, y));
                Ok(())
            }
            Phase::Up(Button::Primary) => {
                let Some(previous) = self.pressed.take() else {
                    return Ok(());
                };
                let Some((x, y)) = point else {
                    return Ok(());
                };
                let target = self.worker.hit(x, y);
                if self.worker.state() == State::Ready && previous.target() == target.target() {
                    let _ = self.worker.submit(Action::Dispatch(target))?;
                }
                Ok(())
            }
            Phase::Cancel => {
                self.pressed = None;
                Ok(())
            }
        }
    }

    fn hover(&mut self) -> Result<(), Fault> {
        if self.worker.state() != State::Ready || self.worker.presented().is_none() {
            return Ok(());
        }
        let next = self.pointer.map(|(x, y)| self.worker.hit(x, y));
        let same =
            self.hovered.as_ref().and_then(Hit::target) == next.as_ref().and_then(Hit::target);
        if !same {
            let _ = self.worker.submit(Action::Transition {
                from: self.hovered.clone(),
                to: next.clone(),
            })?;
        }
        self.hovered = next;
        Ok(())
    }

    fn submit(&mut self) -> Result<(), Fault> {
        if self.worker.state() != State::Ready {
            return Ok(());
        }
        let Some(frame) = self.worker.frame() else {
            return Ok(());
        };
        let context = self.worker.focus(&frame).map(focus);
        self.backend.focus(self.unit, context.as_ref())?;
        if frame.extent().width != self.extent.width || frame.extent().height != self.extent.height
        {
            return Err(Fault("content exceeded assigned extent".into()));
        }
        let result = match self.backend.draw(self.unit, &frame) {
            Ok(graphics::shared::State::Pending) => return Ok(()),
            Ok(graphics::shared::State::Complete) => Ok(()),
            Ok(graphics::shared::State::Failed(fault)) => Err(Fault(fault.0)),
            Err(fault) => Err(fault),
        };
        let _ = self.worker.complete(frame.id(), result)?;
        self.hover()
    }
}

#[cfg(windows)]
pub fn run(bundle: Bundle, options: Options) -> Result<(), Fault> {
    let desktop =
        platform::windows::Desktop::new(&options.renderer, |error| Fault(error.to_string()))?;
    Runner::open(desktop, bundle, options)?.drive()
}

#[cfg(target_os = "macos")]
pub fn run(bundle: Bundle, options: Options) -> Result<(), Fault> {
    let desktop =
        platform::macos::Desktop::new(&options.renderer, |error| Fault(error.to_string()))?;
    Runner::open(desktop, bundle, options)?.drive()
}

#[cfg(target_os = "linux")]
pub fn run(bundle: Bundle, options: Options) -> Result<(), Fault> {
    let desktop =
        platform::linux::Desktop::new(&options.renderer, |error| Fault(error.to_string()))?;
    Runner::open(desktop, bundle, options)?.drive()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn run(_: Bundle, _: Options) -> Result<(), Fault> {
    Err(Fault(
        "native desktop adapter is unavailable on this platform".into(),
    ))
}
