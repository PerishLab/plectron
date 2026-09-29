use super::{Action, Worker};
use crate::Fault;

impl Worker {
    pub(super) fn request(&mut self) -> Result<(), Fault> {
        if self.demand.is_some() || self.fault.is_some() {
            return Ok(());
        }
        let (Some(layout), Some(scene)) = (&self.display.layout, &self.display.scene) else {
            return Ok(());
        };
        if layout.boundaries().is_empty() {
            return Ok(());
        }
        let key = (scene.id(), self.display.visual.clone());
        if self.requested.as_ref() == Some(&key) {
            return Ok(());
        }
        let changed = self
            .requested
            .as_ref()
            .is_some_and(|(_, visual)| *visual != key.1);
        let missing = self
            .display
            .presentation
            .pending
            .as_ref()
            .is_some_and(|frame| !layout.ready(frame));
        if changed || missing {
            self.demand = Some(self.submit(Action::Demand(key.1.clone()))?);
        }
        self.requested = Some(key);
        Ok(())
    }
}
