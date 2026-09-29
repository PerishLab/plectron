use super::{Animations, Plan, Schedule, Timing};

impl Animations {
    pub fn schedule(&self, now: u64) -> Option<Schedule> {
        let mut deadline = None;
        for entry in self.entries.values() {
            let Some(schedule) = entry.schedule(now) else {
                continue;
            };
            match schedule {
                Schedule::Continuous => return Some(Schedule::Continuous),
                Schedule::At(at) => {
                    deadline = Some(deadline.map_or(at, |current: u64| current.min(at)));
                }
            }
        }
        deadline.map(Schedule::At)
    }
}

impl Plan {
    pub fn schedule(&self, now: u64) -> Option<Schedule> {
        if self.paused.is_some() || self.animation.duration == 0 {
            return None;
        }
        if self
            .frames
            .sample(&self.animation, now, self.start, self.base)
            != self.value
        {
            return Some(Schedule::At(now));
        }
        if !matches!(self.animation.timing, Timing::Steps(_, _)) {
            return self.running(now).then_some(Schedule::Continuous);
        }
        self.deadline(now).map(Schedule::At)
    }

    pub fn running(&self, now: u64) -> bool {
        match self.animation.iterations {
            None => true,
            Some(count) => {
                let end = self.start as f64
                    + self.animation.delay as f64
                    + self.animation.duration as f64 * count;
                (now as f64) < end
            }
        }
    }

    fn deadline(&self, now: u64) -> Option<u64> {
        if !self.running(now) {
            return None;
        }
        let active = i128::from(self.start) + i128::from(self.animation.delay);
        let duration = i128::from(self.animation.duration);
        if i128::from(now) < active {
            let at = u64::try_from(active.max(0)).ok()?;
            if self
                .frames
                .sample(&self.animation, at, self.start, self.base)
                != self.value
            {
                return Some(at.max(now.saturating_add(1)));
            }
        }
        let elapsed = (i128::from(now) - active).max(0);
        let iteration = elapsed / duration;
        for cycle in iteration..=iteration.saturating_add(1) {
            for offset in self.boundaries() {
                let point = active
                    + cycle.saturating_mul(duration)
                    + (offset * self.animation.duration as f64).ceil() as i128;
                if point <= i128::from(now) || point < 0 {
                    continue;
                }
                let at = u64::try_from(point).ok()?;
                if !self.running(at)
                    && self
                        .animation
                        .iterations
                        .is_none_or(|count| point as f64 > active as f64 + duration as f64 * count)
                {
                    continue;
                }
                if self
                    .frames
                    .sample(&self.animation, at, self.start, self.base)
                    != self.value
                {
                    return Some(at);
                }
            }
        }
        if let Some(count) = self.animation.iterations {
            let point = active as f64 + self.animation.duration as f64 * count;
            if point > now as f64 && point <= u64::MAX as f64 {
                let at = point.ceil() as u64;
                if self
                    .frames
                    .sample(&self.animation, at, self.start, self.base)
                    != self.value
                {
                    return Some(at);
                }
            }
        }
        None
    }

    fn boundaries(&self) -> Vec<f64> {
        let mut points = Vec::with_capacity(self.frames.opacity.len() + 2);
        if self
            .frames
            .opacity
            .first()
            .is_none_or(|(offset, _)| *offset != 0.0)
        {
            points.push((0.0, self.base));
        }
        points.extend(self.frames.opacity.iter().copied());
        if points.last().is_none_or(|(offset, _)| *offset != 1.0) {
            points.push((1.0, self.base));
        }
        let Timing::Steps(count, start) = self.animation.timing else {
            return Vec::new();
        };
        let mut boundaries = Vec::new();
        for pair in points.windows(2) {
            let (from, _) = pair[0];
            let (to, _) = pair[1];
            let first = usize::from(!start);
            let last = usize::from(count) - usize::from(start);
            for step in first..=last {
                boundaries.push(from + (to - from) * step as f64 / f64::from(count));
            }
        }
        boundaries.push(1.0);
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup_by(|left, right| *left == *right);
        boundaries
    }
}
