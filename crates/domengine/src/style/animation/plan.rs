use super::{Animation, Keyframes};

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub(super) animation: Animation,
    pub(super) frames: Keyframes,
    pub(super) start: u64,
    pub(super) paused: Option<u64>,
    pub(super) base: u8,
    pub(super) value: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track<T> {
    pub target: T,
    pub plan: Plan,
}

impl Plan {
    pub fn value(&self) -> u8 {
        self.value
    }
    pub fn sample(&self, now: u64) -> u8 {
        self.frames.sample(
            &self.animation,
            self.paused.unwrap_or(now),
            self.start,
            self.base,
        )
    }
    pub fn advance(&mut self, now: u64) -> Option<u8> {
        let value = self.sample(now);
        if value == self.value {
            return None;
        }
        self.value = value;
        Some(value)
    }
}
