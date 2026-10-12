use super::{Turn, Worker};
use crate::{Display, Fault};
use std::ops::Deref;

impl Deref for Worker {
    type Target = Display;
    fn deref(&self) -> &Display {
        &self.display
    }
}

impl Worker {
    pub(super) fn flush(&mut self) -> Result<(), Fault> {
        if self.resizing.is_some() {
            return Ok(());
        }
        let changes = self.display.pending();
        if changes.is_empty() {
            return Ok(());
        }
        if !changes.iter().any(|change| change.notify) {
            self.display.take();
            return Ok(());
        }
        self.enqueue(Turn::Scroll(changes, self.display.visual.clone()))?;
        self.display.take();
        Ok(())
    }

    pub fn frame(&mut self) -> Option<graphics::shared::Frame> {
        self.display.frame()
    }
    pub fn acknowledge(&mut self, frame: u64) -> Result<bool, Fault> {
        self.display.acknowledge(frame)
    }
    pub fn complete(&mut self, frame: u64, result: Result<(), Fault>) -> Result<bool, Fault> {
        self.display.complete(frame, result)
    }
    pub fn scroll(&mut self, delta: i64) -> Result<(), Fault> {
        self.display.scroll(delta)?;
        self.request()
    }
    pub fn pan(&mut self, scene: u64, space: u64, delta: i64) -> Result<(), Fault> {
        self.display.pan(scene, space, delta)?;
        self.request()
    }
    pub fn wheel(&mut self, x: usize, y: usize, delta: i64) -> Result<crate::Disposition, Fault> {
        let result = self.display.wheel(x, y, delta)?;
        if result.applied() {
            self.request()?;
        }
        Ok(result)
    }
}
