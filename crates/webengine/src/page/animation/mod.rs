use super::Content;
use crate::Fault;
mod animator;
mod executor;
pub(super) use animator::Animator;
pub(crate) use executor::{Controller, Reader, spawn};

impl Content {
    pub fn render(&mut self) -> Result<(), Fault> {
        self.timing = Default::default();
        self.begin();
        let start = std::time::Instant::now();
        let runtime = self.runtime()?;
        runtime
            .checkpoint()
            .map_err(|failure| runtime.diagnostic(&failure))?;
        self.timing.checkpoint = super::elapsed(start);
        let start = std::time::Instant::now();
        if self.maintain()? {
            self.timing.collections = 1;
            self.timing.collection = super::elapsed(start);
        }
        let replacement = self.runtime.as_mut().and_then(|runtime| {
            let sheet = runtime.addons().sheet();
            (sheet.revision() != self.version).then(|| sheet.clone())
        });
        if let Some(sheet) = replacement {
            self.version = sheet.revision();
            self.capture.replace(sheet);
            self.dirty = true;
        }
        let now = self.clock.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.capture.advance(now);
        if self.build()? {
            let changed = self.settle()?;
            self.converge(changed)?;
        }
        Ok(())
    }
    pub(crate) fn pulse(&mut self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        if self.animation.is_some() {
            return Ok(false);
        }
        let now = pulse
            .target()
            .checked_duration_since(self.clock)
            .unwrap_or_default()
            .as_micros()
            .min(u128::from(u64::MAX)) as u64;
        let due = match self.animator.schedule(now) {
            Some(domengine::style::animation::Schedule::Continuous) => true,
            Some(domengine::style::animation::Schedule::At(at)) => now >= at,
            None => false,
        };
        if !due {
            return Ok(false);
        }
        self.computation.pulses = self.computation.pulses.saturating_add(1);
        self.sample(now)
    }
    fn sample(&mut self, now: u64) -> Result<bool, Fault> {
        let start = std::time::Instant::now();
        let Some(effects) = self.animator.advance(now).map_err(crate::error)? else {
            self.computation.sampling = self
                .computation
                .sampling
                .saturating_add(super::elapsed(start));
            return Ok(false);
        };
        self.computation.sampling = self
            .computation
            .sampling
            .saturating_add(super::elapsed(start));
        self.computation.animations = self.computation.animations.saturating_add(1);
        self.computation.effects = self
            .computation
            .effects
            .saturating_add(effects.values().len() as u64);
        self.effects = Some(effects);
        Ok(true)
    }
}
