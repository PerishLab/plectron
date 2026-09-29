use crate::{Attachment, Delta, Entity, Error, Facts, Id, Point, Signature, Size, Stamp, Store};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timing {
    pub validation: u64,
    pub identity: u64,
    pub delta: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    size: Size,
    overflow: Size,
    clipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Estimated,
    Measured,
    Stale,
}

impl Bounds {
    pub const fn new(size: Size, overflow: Size, clipped: bool) -> Self {
        Self {
            size,
            overflow,
            clipped,
        }
    }

    pub const fn size(self) -> Size {
        self.size
    }

    pub const fn overflow(self) -> Size {
        self.overflow
    }

    pub const fn clipped(self) -> bool {
        self.clipped
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metric {
    pub id: Id,
    origin: Point,
    bounds: Bounds,
    attachment: Option<Attachment>,
    quality: Quality,
}

impl Metric {
    pub const fn new(
        id: Id,
        origin: Point,
        content: (Bounds, Option<Attachment>, Quality),
    ) -> Self {
        let (bounds, attachment, quality) = content;
        Self {
            id,
            origin,
            bounds,
            attachment,
            quality,
        }
    }

    pub const fn origin(&self) -> Point {
        self.origin
    }

    pub const fn size(&self) -> Size {
        self.bounds.size()
    }

    pub const fn overflow(&self) -> Size {
        self.bounds.overflow()
    }

    pub const fn clipped(&self) -> bool {
        self.bounds.clipped()
    }

    pub const fn attachment(&self) -> Option<Attachment> {
        self.attachment
    }

    pub const fn quality(&self) -> Quality {
        self.quality
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Resolved(Metric),
    Deferred(Metric),
    Blocked(Id),
}

impl State {
    pub const fn metric(&self) -> Option<&Metric> {
        match self {
            Self::Resolved(metric) | Self::Deferred(metric) => Some(metric),
            Self::Blocked(_) => None,
        }
    }

    pub const fn boundary(&self) -> Option<Id> {
        match self {
            Self::Blocked(boundary) => Some(*boundary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    generation: u64,
    facts: Arc<Facts>,
    states: Store<State>,
    identity: crate::identity::Identity,
    delta: Delta,
    timing: Timing,
}

#[derive(Debug, Clone, Copy)]
pub struct Entry<'a> {
    layout: &'a Layout,
    slot: usize,
}

impl Layout {
    pub fn new(
        facts: Arc<Facts>,
        states: Store<State>,
        previous: Option<&Self>,
        scope: Option<&[Id]>,
    ) -> Result<Arc<Self>, Error> {
        let start = std::time::Instant::now();
        validate(&facts, &states, scope)?;
        let validation = elapsed(start);
        let start = std::time::Instant::now();
        let identity = crate::identity::Identity::build(
            &facts,
            &states,
            previous.map(|layout| crate::identity::History {
                facts: &layout.facts,
                states: &layout.states,
                identity: &layout.identity,
            }),
        )?;
        let publication = elapsed(start);
        let history = previous.map(|layout| (layout.facts.as_ref(), &layout.states));
        let start = std::time::Instant::now();
        let delta = Delta::build(&facts, &states, history, scope);
        let changes = elapsed(start);
        Ok(Arc::new(Self {
            generation: NEXT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .expect("geometry layout generation exhausted"),
            facts,
            states,
            identity,
            delta,
            timing: Timing {
                validation,
                identity: publication,
                delta: changes,
            },
        }))
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn facts(&self) -> &Arc<Facts> {
        &self.facts
    }

    pub const fn states(&self) -> &Store<State> {
        &self.states
    }

    pub const fn delta(&self) -> &Delta {
        &self.delta
    }

    pub const fn publication(&self) -> crate::Publication {
        self.identity.publication()
    }

    pub const fn timing(&self) -> Timing {
        self.timing
    }

    pub fn shape(&self) -> crate::Shape {
        self.facts.shape()
    }

    pub fn residency(&self) -> crate::Residency {
        self.states.residency()
    }

    pub fn state(&self, id: Id) -> Option<&State> {
        self.facts.slot(id).and_then(|slot| self.states.get(slot))
    }

    pub fn metric(&self, id: Id) -> Option<&Metric> {
        self.state(id).and_then(State::metric)
    }

    pub fn entry(&self, id: Id) -> Option<Entry<'_>> {
        self.facts.slot(id).map(|slot| Entry { layout: self, slot })
    }

    pub fn root(&self) -> Entry<'_> {
        let slot = self
            .facts
            .slot(self.facts.root())
            .expect("validated geometry root");
        Entry { layout: self, slot }
    }
}

fn elapsed(start: std::time::Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

impl<'a> Entry<'a> {
    pub const fn slot(self) -> usize {
        self.slot
    }

    pub fn entity(self) -> &'a Entity {
        &self.layout.facts.entities()[self.slot]
    }

    pub fn state(self) -> &'a State {
        &self.layout.states[self.slot]
    }

    pub fn stamp(self) -> Stamp {
        self.layout.identity.stamp(self.slot)
    }

    pub fn metric(self) -> Option<&'a Metric> {
        self.state().metric()
    }

    pub fn signature(self) -> Option<&'a Signature> {
        self.layout.identity.signature(self.slot)
    }

    pub fn children(self) -> impl Iterator<Item = Self> + 'a {
        self.layout.facts.children(self.slot).map(move |slot| Self {
            layout: self.layout,
            slot,
        })
    }
}

fn validate(facts: &Facts, states: &Store<State>, scope: Option<&[Id]>) -> Result<(), Error> {
    if states.count() != facts.entities().len() {
        return Err(Error(
            "geometry state count does not match its facts".into(),
        ));
    }
    if facts
        .entities()
        .iter()
        .zip(states.iter())
        .any(|(entity, state)| state.metric().is_some_and(|metric| metric.id != entity.id))
    {
        return Err(Error(
            "geometry state identity does not match its facts".into(),
        ));
    }
    if scope.is_some_and(|scope| scope.iter().any(|id| facts.slot(*id).is_none())) {
        return Err(Error(
            "geometry change scope contains an absent identity".into(),
        ));
    }
    Ok(())
}
