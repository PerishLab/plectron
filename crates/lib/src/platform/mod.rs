use std::collections::{BTreeMap, VecDeque};

use crate::contract::{Event, Fault, Signal, Unit, Window};
use crate::rendering::Renderer;
use crate::resource::Services;
pub mod device;
mod instance;
pub use instance::Evaluation;
use instance::Instance;
mod content;
mod output;
pub use output::Receipt;
mod ports;
pub use ports::{Backend, Console, Display, Input};

pub trait Host {
    fn window(&mut self, options: Window) -> Result<Unit, Fault>;
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), Fault>;
    fn fill(&mut self, unit: Unit, color: u32) -> Result<(), Fault>;
    fn close(&mut self, unit: Unit) -> Result<(), Fault>;
    fn check(&self, unit: Unit) -> Result<(), Fault>;
    fn log(&mut self, text: &str) -> Result<(), Fault>;
    fn serve(&mut self, root: &str) -> Result<u64, Fault>;
    fn scheme(&mut self, service: u64, origin: &str) -> Result<(), Fault>;
    fn stop(&mut self, service: u64) -> Result<(), Fault>;
    fn attach(&mut self, unit: Unit, url: &str) -> Result<(), Fault>;
    fn message(&mut self, unit: Unit, text: &str) -> Result<(), Fault>;
    fn input(&mut self, signal: &Signal) -> Result<bool, Fault>;
    fn finish(&mut self) -> Result<(), Fault>;
}

pub struct Platform<B, R> {
    query: crate::ipc::query::Store,
    config: config::Store,
    backend: B,
    renderer: R,
    units: BTreeMap<Unit, Window>,
    pending: VecDeque<Pending>,
    sequence: u64,
    sealed: bool,
    services: Services,
    pages: BTreeMap<Unit, Instance>,
    pointers: BTreeMap<Unit, Option<(usize, usize)>>,
    hovered: BTreeMap<Unit, Option<webengine::Hit>>,
    presses: BTreeMap<Unit, Press>,
    threaded: bool,
    notify: Option<shared::Notify>,
    incoming: VecDeque<(Unit, String)>,
}

impl<B: Backend, R: Renderer> Platform<B, R> {
    pub fn new(backend: B, renderer: R) -> Self {
        Self::observed(backend, renderer, Default::default())
    }

    pub fn observed(backend: B, renderer: R, query: crate::ipc::query::Store) -> Self {
        Self {
            query,
            config: config::Store::default(),
            backend,
            renderer,
            units: BTreeMap::new(),
            pending: VecDeque::new(),
            sequence: 0,
            sealed: false,
            services: Services::default(),
            pages: BTreeMap::new(),
            pointers: BTreeMap::new(),
            hovered: BTreeMap::new(),
            presses: BTreeMap::new(),
            threaded: false,
            notify: None,
            incoming: VecDeque::new(),
        }
    }

    pub fn query(&self) -> crate::ipc::query::Store {
        self.query.clone()
    }

    pub fn notifier(&self) -> shared::Notify {
        self.backend.notifier()
    }

    pub fn prepare(&mut self) -> Result<(), Fault> {
        self.backend.prepare()
    }

    pub fn wait(&mut self) -> Result<(), Fault> {
        let mut pacing = graphics::shared::Pacing::Idle;
        for page in self.pages.values() {
            pacing = pacing.combine(page.pacing()?);
        }
        self.backend.wait(pacing)
    }

    pub fn notify(&mut self, notify: Option<shared::Notify>) {
        self.notify = notify;
    }

    pub fn config(&self) -> config::Store {
        self.config.clone()
    }

    pub fn threaded(backend: B, renderer: R) -> Self {
        let mut platform = Self::new(backend, renderer);
        platform.threaded = true;
        platform
    }
    pub fn workers(mut self) -> Self {
        self.threaded = true;
        self
    }
    pub fn windows(&self) -> Vec<(Unit, Window)> {
        self.units
            .iter()
            .map(|(unit, window)| (*unit, window.clone()))
            .collect()
    }

    pub fn unit(&self, id: u64) -> Result<Unit, Fault> {
        let unit = Unit::new(id);
        self.check(unit)?;
        Ok(unit)
    }

    pub fn shutdown(&mut self) -> (Vec<Signal>, Vec<Fault>) {
        self.sealed = true;
        let mut errors = Vec::new();
        for (unit, _) in self.windows() {
            if let Err(error) = self.close(unit) {
                errors.push(error);
            }
        }
        self.incoming.clear();
        let signals = self
            .pending
            .drain(..)
            .filter_map(|pending| match pending {
                Pending::Closed(unit) => Some(Signal {
                    unit,
                    event: Event::Close,
                }),
                Pending::Native(_) => None,
            })
            .collect();
        (signals, errors)
    }

    pub fn active(&self) -> bool {
        !self.units.is_empty() || !self.pending.is_empty() || !self.incoming.is_empty()
    }

    pub fn poll(&mut self) -> Result<Vec<Signal>, Fault> {
        if self.pending.is_empty() {
            for signal in self.backend.poll()? {
                self.admit(signal)?;
            }
        }
        self.progress()?;
        while let Some(pending) = self.pending.pop_front() {
            let signal = pending.signal();
            if matches!(pending, Pending::Closed(_)) {
                return Ok(vec![signal]);
            }
            if self.units.contains_key(&signal.unit) {
                self.accept(&signal)?;
                return Ok(vec![signal]);
            }
        }
        Ok(Vec::new())
    }

    fn accept(&mut self, signal: &Signal) -> Result<(), Fault> {
        if signal.event == Event::Close {
            self.backend.close(signal.unit)?;
            self.units.remove(&signal.unit);
            self.dispose(signal.unit);
            self.pending
                .retain(|pending| pending.signal().unit != signal.unit);
        }
        Ok(())
    }
}

impl<B: Backend, R: Renderer> Host for Platform<B, R> {
    fn window(&mut self, options: Window) -> Result<Unit, Fault> {
        if self.sealed {
            return Err(Fault("platform is shutting down".into()));
        }
        options
            .extent
            .area()
            .map_err(|error| Fault(error.to_string()))?;
        if self.units.len() >= 8 {
            return Err(Fault("window budget exhausted".into()));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Fault("resource IDs exhausted".into()))?;
        let unit = Unit::new(self.sequence);
        self.backend.create(unit, &options)?;
        self.units.insert(unit, options);
        Ok(unit)
    }

    fn title(&mut self, unit: Unit, title: &str) -> Result<(), Fault> {
        self.check(unit)?;
        self.backend.title(unit, title)?;
        self.units.get_mut(&unit).expect("checked unit").title = title.into();
        Ok(())
    }

    fn fill(&mut self, unit: Unit, color: u32) -> Result<(), Fault> {
        self.check(unit)?;
        if self.pages.contains_key(&unit) {
            return Err(Fault("web content owns this surface".into()));
        }
        self.backend.activate(unit)?;
        let extent = self.units[&unit].extent;
        let frame = self.renderer.paint(extent, color)?;
        let pixels = extent.area().map_err(|error| Fault(error.to_string()))?;
        if frame.extent != extent || frame.pixels.len() != pixels {
            return Err(Fault("renderer exceeded its assigned extent".into()));
        }
        self.backend.present(unit, frame)
    }

    fn close(&mut self, unit: Unit) -> Result<(), Fault> {
        self.check(unit)?;
        self.backend.close(unit)?;
        self.units.remove(&unit);
        self.dispose(unit);
        self.pending.retain(|pending| pending.signal().unit != unit);
        self.pending.push_back(Pending::Closed(unit));
        Ok(())
    }

    fn check(&self, unit: Unit) -> Result<(), Fault> {
        if self.units.contains_key(&unit) {
            Ok(())
        } else {
            Err(Fault(format!("window {} is closed or unknown", unit.id())))
        }
    }

    fn log(&mut self, text: &str) -> Result<(), Fault> {
        self.backend.log(text)
    }
    fn serve(&mut self, root: &str) -> Result<u64, Fault> {
        if self.sealed {
            return Err(Fault("platform is shutting down".into()));
        }
        self.services.serve(std::path::Path::new(root))
    }
    fn scheme(&mut self, service: u64, origin: &str) -> Result<(), Fault> {
        self.services.scheme(service, origin)
    }
    fn stop(&mut self, service: u64) -> Result<(), Fault> {
        self.services.close(service)
    }
    fn attach(&mut self, unit: Unit, url: &str) -> Result<(), Fault> {
        self.load(unit, url)
    }
    fn message(&mut self, unit: Unit, text: &str) -> Result<(), Fault> {
        self.enqueue(unit, text)
    }
    fn input(&mut self, signal: &Signal) -> Result<bool, Fault> {
        self.route(signal)
    }
    fn finish(&mut self) -> Result<(), Fault> {
        self.advance()
    }
}

enum Pending {
    Native(Signal),
    Closed(Unit),
}

struct Press {
    content: bool,
    target: Option<webengine::Hit>,
}

impl Pending {
    fn signal(&self) -> Signal {
        match self {
            Self::Native(signal) => signal.clone(),
            Self::Closed(unit) => Signal {
                unit: *unit,
                event: Event::Close,
            },
        }
    }
}
