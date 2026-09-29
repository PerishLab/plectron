use crate::Fault;
mod parse;
mod plan;
mod schedule;
use crate::Node;
pub(super) use parse::Parse;
pub use plan::{Plan, Track};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub name: Option<String>,
    pub duration: u64,
    pub delay: i64,
    pub iterations: Option<f64>,
    pub timing: Timing,
    pub playing: bool,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: None,
            duration: 0,
            delay: 0,
            iterations: Some(1.0),
            timing: Timing::Ease,
            playing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    Ease,
    Linear,
    Steps(u16, bool),
}

impl Timing {
    pub fn parse(value: &str) -> Result<Self, Fault> {
        match value {
            "ease" => Ok(Self::Ease),
            "linear" => Ok(Self::Linear),
            "step-start" => Ok(Self::Steps(1, true)),
            "step-end" => Ok(Self::Steps(1, false)),
            _ => {
                let inside = value
                    .strip_prefix("steps(")
                    .and_then(|value| value.strip_suffix(')'))
                    .ok_or_else(|| Fault("unsupported animation timing function".into()))?;
                let (count, position) = inside
                    .split_once(',')
                    .map_or((inside, "end"), |(count, position)| {
                        (count, position.trim())
                    });
                let count = count
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|count| (1..=256).contains(count))
                    .ok_or_else(|| Fault("invalid animation step count".into()))?;
                let start = match position {
                    "start" | "jump-start" => true,
                    "end" | "jump-end" => false,
                    _ => return Err(Fault("unsupported animation step position".into())),
                };
                Ok(Self::Steps(count, start))
            }
        }
    }

    fn progress(self, value: f64) -> f64 {
        match self {
            Self::Linear => value,
            Self::Steps(count, start) => {
                let count = f64::from(count);
                ((value * count).floor() + f64::from(start as u8)).clamp(0.0, count) / count
            }
            Self::Ease => ease(value),
        }
    }
}

fn ease(value: f64) -> f64 {
    let (mut low, mut high): (f64, f64) = (0.0, 1.0);
    for _ in 0..20 {
        let t = (low + high) / 2.0;
        let x = 3.0 * (1.0 - t).powi(2) * t * 0.25 + 3.0 * (1.0 - t) * t * t * 0.25 + t.powi(3);
        if x < value {
            low = t;
        } else {
            high = t;
        }
    }
    let t = (low + high) / 2.0;
    3.0 * (1.0 - t).powi(2) * t * 0.1 + 3.0 * (1.0 - t) * t * t + t.powi(3)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes {
    pub id: u64,
    pub revision: u64,
    pub name: String,
    pub opacity: Vec<(f64, u8)>,
}

#[derive(Default)]
pub struct Animations {
    entries: BTreeMap<Node, Plan>,
    changes: BTreeMap<Node, u8>,
    now: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub node: Node,
    pub opacity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Continuous,
    At(u64),
}

impl Animations {
    pub fn advance(&mut self, now: u64) -> bool {
        self.now = now;
        let mut changed = false;
        for (node, plan) in &mut self.entries {
            if let Some(value) = plan.advance(now) {
                self.changes.insert(*node, value);
                changed = true;
            }
        }
        changed
    }

    pub fn changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
            .into_iter()
            .map(|(node, opacity)| Change { node, opacity })
            .collect()
    }

    pub fn discard(&mut self) {
        self.changes.clear();
    }

    pub fn running(&self) -> bool {
        self.entries.values().any(|plan| plan.running(self.now))
    }

    pub fn sample(
        &mut self,
        node: Node,
        animation: &Animation,
        frames: &Keyframes,
        base: u8,
    ) -> u8 {
        let entry = self.entries.entry(node).or_insert_with(|| Plan {
            animation: animation.clone(),
            frames: frames.clone(),
            start: self.now,
            paused: (!animation.playing).then_some(self.now),
            base,
            value: base,
        });
        if entry.animation.name != animation.name {
            entry.start = self.now;
            entry.paused = (!animation.playing).then_some(self.now);
        } else if entry.animation.playing != animation.playing {
            if animation.playing {
                if let Some(paused) = entry.paused.take() {
                    entry.start = entry.start.saturating_add(self.now.saturating_sub(paused));
                }
            } else {
                entry.paused = Some(self.now);
            }
        }
        entry.animation = animation.clone();
        entry.frames = frames.clone();
        entry.base = base;
        let value = entry.sample(self.now);
        if value != entry.value {
            self.changes.insert(node, value);
        }
        entry.value = value;
        entry.value
    }

    pub fn retain(&mut self, mut connected: impl FnMut(Node) -> bool) {
        self.entries.retain(|node, _| connected(*node));
        self.changes.retain(|node, _| connected(*node));
    }

    pub fn cancel(&mut self, node: Node) {
        self.entries.remove(&node);
        self.changes.remove(&node);
    }
    pub fn plans(&self) -> Vec<Track<Node>> {
        self.entries
            .iter()
            .map(|(node, plan)| Track {
                target: *node,
                plan: plan.clone(),
            })
            .collect()
    }
}

impl Keyframes {
    pub fn new(name: String, mut opacity: Vec<(f64, u8)>) -> Result<Self, Fault> {
        Parse::valid(&name)?;
        if opacity.is_empty() || opacity.len() > 64 {
            return Err(Fault("keyframe count must be between 1 and 64".into()));
        }
        opacity.sort_by(|a, b| a.0.total_cmp(&b.0));
        opacity.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                earlier.1 = later.1;
                true
            } else {
                false
            }
        });
        Ok(Self {
            id: 0,
            revision: 0,
            name,
            opacity,
        })
    }

    pub fn sample(&self, animation: &Animation, now: u64, start: u64, base: u8) -> u8 {
        let duration = animation.duration;
        if duration == 0 {
            return base;
        }
        let elapsed = i128::from(now) - i128::from(start) - i128::from(animation.delay);
        if elapsed < 0 {
            return base;
        }
        let elapsed = elapsed as f64;
        let duration = duration as f64;
        if animation
            .iterations
            .is_some_and(|count| elapsed >= duration * count)
        {
            return base;
        }
        let progress = (elapsed % duration) / duration;
        let mut previous = (0.0, base);
        for &(offset, value) in &self.opacity {
            if progress < offset {
                let distance = (progress - previous.0) / (offset - previous.0);
                let amount = animation.timing.progress(distance);
                return (f64::from(previous.1) * (1.0 - amount) + f64::from(value) * amount).round()
                    as u8;
            }
            previous = (offset, value);
        }
        if previous.0 == 1.0 {
            return previous.1;
        }
        let amount = animation
            .timing
            .progress((progress - previous.0) / (1.0 - previous.0));
        (f64::from(previous.1) * (1.0 - amount) + f64::from(base) * amount).round() as u8
    }
}
