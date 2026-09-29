use crate::{Bounds, Error, Facts, Quality, Size, State, Store};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
static ATTACHMENTS: AtomicU64 = AtomicU64::new(1);
static STAMPS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Attachment(u64);

impl Attachment {
    pub fn fresh() -> Self {
        Self(
            ATTACHMENTS
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .expect("geometry attachment identity exhausted"),
        )
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Id {
    pub fn fresh() -> Self {
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("geometry identity exhausted"),
        )
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Source {
    owner: u64,
    slot: u64,
    generation: u64,
}

impl Source {
    pub const fn new(owner: u64, slot: u64, generation: u64) -> Self {
        Self {
            owner,
            slot,
            generation,
        }
    }

    pub const fn slot(self) -> u64 {
        self.slot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp(u64);

impl Stamp {
    fn fresh() -> Self {
        Self(
            STAMPS
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .expect("geometry state stamp exhausted"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Children {
    None,
    One(Stamp),
    Many(Arc<[Stamp]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    deferred: bool,
    bounds: Bounds,
    attachment: Option<Attachment>,
    quality: Quality,
    children: Children,
}

impl Signature {
    fn new(state: &State, mut children: impl Iterator<Item = Stamp>, count: usize) -> Option<Self> {
        let metric = state.metric()?;
        let children = match count {
            0 => Children::None,
            1 => Children::One(children.next().expect("geometry child identity")),
            _ => Children::Many(children.collect::<Vec<_>>().into()),
        };
        Some(Self {
            deferred: matches!(state, State::Deferred(_)),
            bounds: Bounds::new(metric.size(), metric.overflow(), metric.clipped()),
            attachment: metric.attachment(),
            quality: metric.quality(),
            children,
        })
    }

    pub const fn deferred(&self) -> bool {
        self.deferred
    }

    pub const fn size(&self) -> Size {
        self.bounds.size()
    }

    pub const fn bounds(&self) -> Bounds {
        self.bounds
    }

    pub const fn attachment(&self) -> Option<Attachment> {
        self.attachment
    }

    pub const fn quality(&self) -> Quality {
        self.quality
    }

    pub fn children(&self) -> &[Stamp] {
        match &self.children {
            Children::None => &[],
            Children::One(stamp) => std::slice::from_ref(stamp),
            Children::Many(stamps) => stamps,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Publication {
    pub scanned: usize,
    pub compared: usize,
    pub retained: usize,
    pub fresh: usize,
    pub children: usize,
    pub allocated: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Identity {
    stamps: Arc<[Stamp]>,
    signatures: Arc<[Option<Signature>]>,
    publication: Publication,
}

pub(crate) struct History<'a> {
    pub facts: &'a Facts,
    pub states: &'a Store<State>,
    pub identity: &'a Identity,
}

impl Identity {
    pub fn build(
        facts: &Facts,
        states: &Store<State>,
        previous: Option<History<'_>>,
    ) -> Result<Self, Error> {
        let length = states.count();
        let mut stamps = vec![None; length];
        let mut signatures = vec![None; length];
        let mut publication = Publication::default();
        for &slot in facts.postorder() {
            publication.scanned += 1;
            let signature = signature((facts, states), &stamps, slot, &mut publication);
            let retained = previous.as_ref().and_then(|previous| {
                let old = previous.facts.slot(facts.entities()[slot].id)?;
                publication.compared += 1;
                equivalent(
                    &states[slot],
                    signature.as_ref(),
                    &previous.states[old],
                    previous.identity.signature(old),
                )
                .then(|| previous.identity.stamp(old))
            });
            stamps[slot] = Some(retained.unwrap_or_else(|| {
                publication.fresh += 1;
                Stamp::fresh()
            }));
            publication.retained += usize::from(retained.is_some());
            signatures[slot] = signature;
        }
        Ok(Self {
            stamps: stamps
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .expect("completed geometry identity")
                .into(),
            signatures: signatures.into(),
            publication,
        })
    }

    pub fn stamp(&self, slot: usize) -> Stamp {
        self.stamps[slot]
    }

    pub fn signature(&self, slot: usize) -> Option<&Signature> {
        self.signatures[slot].as_ref()
    }

    pub const fn publication(&self) -> Publication {
        self.publication
    }
}

fn signature(
    current: (&Facts, &Store<State>),
    stamps: &[Option<Stamp>],
    slot: usize,
    publication: &mut Publication,
) -> Option<Signature> {
    let (facts, states) = current;
    let state = &states[slot];
    let children = matches!(state, State::Resolved(_))
        .then(|| facts.childslots(slot))
        .unwrap_or_default();
    publication.children += children.len();
    publication.allocated += usize::from(children.len() > 1);
    Signature::new(
        state,
        children
            .iter()
            .map(|child| stamps[*child].expect("resolved child identity")),
        children.len(),
    )
}

fn equivalent(
    state: &State,
    signature: Option<&Signature>,
    previous: &State,
    old: Option<&Signature>,
) -> bool {
    match (state, previous) {
        (State::Blocked(boundary), State::Blocked(old)) => boundary == old,
        _ => signature.is_some() && signature == old,
    }
}
