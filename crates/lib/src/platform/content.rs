use super::{Backend, Host, Pending, Platform};
use super::{Evaluation, Instance};
use crate::contract::{Event, Fault, Signal, Unit};
use crate::rendering::Renderer;
use crate::resource::Service;
use webengine::{Snapshot, State};

impl<B: Backend, R: Renderer> Platform<B, R> {
    pub fn content(&mut self, unit: Unit) -> Result<Snapshot, Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .map(Instance::inspect)
            .ok_or_else(|| Fault("unit has no web content".into()))
    }

    pub fn accepting(&self, unit: Unit) -> bool {
        self.pages.get(&unit).is_some_and(Instance::available)
    }
    pub fn attached(&self, unit: Unit) -> bool {
        self.pages.contains_key(&unit)
    }

    pub fn services(&self) -> Vec<Service> {
        self.services.inspect()
    }

    pub fn cache(
        &mut self,
        unit: Unit,
        id: u64,
        command: shared::cache::Command,
    ) -> Result<Evaluation, Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .cache(id, command)
    }
    pub fn evaluate(&mut self, unit: Unit, source: &str) -> Result<Evaluation, Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .evaluate(source)
    }

    pub fn collect(&mut self, unit: Unit) -> Result<Evaluation, Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .collect()
    }

    pub fn job(&self, unit: u64, id: u64) -> Option<&webengine::Job> {
        self.pages.get(&Unit::new(unit))?.job(id)
    }
    pub(super) fn load(&mut self, unit: Unit, url: &str) -> Result<(), Fault> {
        self.check(unit)?;
        if self.pages.contains_key(&unit) {
            return Err(Fault(
                "unit already has content; navigation is unsupported".into(),
            ));
        }
        let extent = self.units[&unit].extent;
        let page = Instance::new(
            url,
            domengine::Extent {
                width: extent.width,
                height: extent.height,
            },
            self.threaded.then(|| self.services.loader()),
            (self.config.clone(), self.notify.clone()),
        )?;
        self.backend.activate(unit)?;
        self.pages.insert(unit, page);
        Ok(())
    }

    pub(super) fn enqueue(&mut self, unit: Unit, text: &str) -> Result<(), Fault> {
        self.check(unit)?;
        if self.pages.get(&unit).map(|page| page.state()) != Some(State::Ready) {
            return Err(Fault("content is not ready".into()));
        }
        if text.len() > 16_384 || self.incoming.len() >= 256 {
            return Err(Fault("message budget exhausted".into()));
        }
        self.incoming.push_back((unit, text.into()));
        Ok(())
    }

    pub(super) fn route(&mut self, signal: &Signal) -> Result<bool, Fault> {
        if !signal.event.input() {
            return Ok(false);
        }
        let Some(page) = self.pages.get_mut(&signal.unit) else {
            return Ok(false);
        };
        match &signal.event {
            Event::Input(input) => {
                if page.state() == State::Ready {
                    page.input(input)?;
                }
                Ok(true)
            }
            Event::Edit(edit) => {
                if page.state() == State::Ready {
                    page.edit(edit)?;
                }
                Ok(true)
            }
            Event::Composition(input) => {
                if page.state() == State::Ready {
                    page.compose(input)?;
                }
                Ok(true)
            }
            Event::Pointer { x, y } => {
                if page.state() == State::Ready && page.presented().is_some() {
                    let hit = page.hit(*x, *y);
                    page.dispatch(&hit)?;
                }
                Ok(true)
            }
            Event::Target(hit) => {
                if page.state() == State::Ready {
                    page.dispatch(hit)?;
                }
                Ok(true)
            }
            _ => Err(Fault("web content requires pointer coordinates".into())),
        }
    }

    pub(super) fn advance(&mut self) -> Result<(), Fault> {
        for unit in self.pages.keys().copied().collect::<Vec<_>>() {
            self.present(unit)?;
        }
        Ok(())
    }

    fn present(&mut self, unit: Unit) -> Result<(), Fault> {
        let result = self
            .pages
            .get_mut(&unit)
            .expect("owned page")
            .advance(&mut self.services);
        match result {
            Ok(events) => self.events(unit, events),
            Err(error) if self.pages[&unit].state() == State::Failed => {
                self.events(unit, vec![Event::Error(error.to_string())])
            }
            Err(error) => return Err(error),
        }
        self.submit(unit)
    }
    pub(super) fn events(&mut self, unit: Unit, events: Vec<Event>) {
        self.pending.extend(
            events
                .into_iter()
                .map(|event| Pending::Native(Signal { unit, event })),
        );
    }
    pub fn hold(&mut self, unit: Unit) -> Result<u64, Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .hold()
    }
    pub fn unhold(&mut self, unit: Unit) -> Result<(), Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .unhold()
    }
    pub fn release(&mut self) {
        self.services.clear();
        self.incoming.clear();
    }

    pub fn waiting(&self) -> usize {
        self.incoming.len()
    }

    pub fn delivery(&mut self) -> Result<bool, Fault> {
        let Some((unit, text)) = self.incoming.pop_front() else {
            return Ok(false);
        };
        if let Some(page) = self.pages.get_mut(&unit) {
            page.receive(text)?;
        }
        self.advance()?;
        Ok(true)
    }

    pub(super) fn dispose(&mut self, unit: Unit) {
        self.query.cancel(Some(unit.id()));
        if let Some(mut page) = self.pages.remove(&unit) {
            page.close();
        }
        self.pointers.remove(&unit);
        self.hovered.remove(&unit);
        self.presses.remove(&unit);
        self.incoming.retain(|(target, _)| *target != unit);
    }
}
