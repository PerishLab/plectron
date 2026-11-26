use std::collections::VecDeque;
use std::time::Instant;

const AWAITED: usize = 64;

struct Entry {
    frame: u64,
    refresh: u64,
    produced: Instant,
}

#[derive(Default)]
pub(super) struct Sightings {
    awaited: VecDeque<Entry>,
    seen: Vec<(u64, Instant)>,
}

impl Sightings {
    pub fn watch(&mut self, frame: u64, refresh: u64, produced: Instant) {
        if self.awaited.len() == AWAITED {
            self.awaited.pop_front();
        }
        self.awaited.push_back(Entry {
            frame,
            refresh,
            produced,
        });
    }

    pub fn awaiting(&self) -> bool {
        !self.awaited.is_empty()
    }

    pub fn settle(&mut self, refresh: u64, visible: Instant, shown: bool) {
        while let Some(entry) = self.awaited.front() {
            if refresh <= entry.refresh {
                break;
            }
            if let Some(entry) = self.awaited.pop_front()
                && shown
            {
                self.seen.push((entry.frame, visible.max(entry.produced)));
            }
        }
    }

    pub fn clear(&mut self) {
        self.awaited.clear();
    }

    pub fn take(&mut self) -> Vec<(u64, Instant)> {
        std::mem::take(&mut self.seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn settlement() {
        let mut sightings = Sightings::default();
        let base = Instant::now();
        let later = base + Duration::from_millis(5);
        sightings.watch(1, 10, later);
        sightings.watch(2, 11, base);
        assert!(sightings.awaiting());
        sightings.settle(10, later, true);
        assert!(sightings.take().is_empty());
        assert!(sightings.awaiting());
        sightings.settle(11, base, true);
        assert_eq!(sightings.take(), vec![(1, later)]);
        assert!(sightings.awaiting());
        sightings.settle(12, later, false);
        assert!(sightings.take().is_empty());
        assert!(!sightings.awaiting());
        sightings.watch(3, 12, base);
        sightings.settle(13, later, true);
        assert_eq!(sightings.take(), vec![(3, later)]);
    }
}
