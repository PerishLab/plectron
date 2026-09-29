use crate::shared::{Fault, Item, Scene, operation::Source};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const CHECKS: usize = 64;
static UPDATE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub space: u64,
    pub opacity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    id: u64,
    generation: u64,
    values: Arc<[Effect]>,
}

impl Effects {
    pub fn new(generation: u64, values: Vec<Effect>) -> Result<Self, Fault> {
        let id = UPDATE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Fault("effect identifiers exhausted".into()))?;
        Ok(Self {
            id,
            generation,
            values: values.into(),
        })
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn values(&self) -> &[Effect] {
        &self.values
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Routing {
    pub checked: usize,
    pub retained: usize,
}

struct Proof {
    remaining: usize,
    checked: usize,
}

impl Scene {
    pub fn animate(&self, effects: &Effects) -> Result<Option<Self>, Fault> {
        if effects.generation != self.generation {
            return Ok(None);
        }
        let mut values = (*self.effects).clone();
        let mut changed = 0;
        for effect in effects.values() {
            let base = self
                .spaces
                .opacity(effect.space)
                .ok_or_else(|| Fault("animation targets an unknown visual space".into()))?;
            let before = values.get(&effect.space).copied().unwrap_or(base);
            if effect.opacity == base {
                values.remove(&effect.space);
            } else {
                values.insert(effect.space, effect.opacity);
            }
            changed += usize::from(before != effect.opacity);
        }
        if changed == 0 {
            return Ok(None);
        }
        Ok(Some(Self {
            id: super::super::identifier()?,
            generation: self.generation,
            extent: self.extent,
            revision: self.revision,
            items: self.items.clone(),
            spaces: self.spaces.clone(),
            effects: Arc::new(values),
            routing: Routing {
                checked: effects.values().len(),
                retained: 1,
            },
        }))
    }

    pub fn revise(&self, source: Source, limit: usize) -> Result<Self, Fault> {
        let mut proof = Proof {
            remaining: limit,
            checked: 0,
        };
        let retained = source.extent == self.extent && proof.items(&self.items, &source.items);
        let mut scene = if retained {
            let id = super::super::identifier()?;
            Self {
                id,
                generation: id,
                extent: source.extent,
                revision: source.revision,
                items: source.items.into(),
                spaces: self.spaces.clone(),
                effects: Default::default(),
                routing: Routing::default(),
            }
        } else {
            Self::publish(source.extent, source.revision, source.items)?
        };
        scene.routing = Routing {
            checked: proof.checked,
            retained: usize::from(retained),
        };
        Ok(scene)
    }
    pub fn routing(&self) -> Routing {
        self.routing
    }
}

impl Proof {
    fn items(&mut self, before: &[Item], after: &[Item]) -> bool {
        before.len() == after.len() && before.iter().zip(after).all(|(a, b)| self.item(a, b))
    }
    fn item(&mut self, before: &Item, after: &Item) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.checked += 1;
        match (before, after) {
            (
                Item::Group {
                    space: a,
                    items: left,
                    ..
                },
                Item::Group {
                    space: b,
                    items: right,
                    ..
                },
            ) => a == b && (Arc::ptr_eq(left, right) || self.items(left, right)),
            (
                Item::Draw {
                    origin: a,
                    clip: c,
                    record: left,
                },
                Item::Draw {
                    origin: b,
                    clip: d,
                    record: right,
                },
            ) => a == b && c == d && Arc::ptr_eq(left, right),
            (Item::Fill { rect: a, color: c }, Item::Fill { rect: b, color: d }) => {
                a == b && c == d
            }
            _ => false,
        }
    }
}
