use crate::{Facts, Id, State, Store};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scan {
    pub compared: usize,
    pub scanned: usize,
    pub indexed: usize,
}

#[derive(Debug, Clone)]
pub struct Delta {
    changed: Arc<[Id]>,
    added: Arc<[Id]>,
    moved: Arc<[Id]>,
    resized: Arc<[Id]>,
    overflow: Arc<[Id]>,
    clipped: Arc<[Id]>,
    pending: Arc<[Id]>,
    retired: Arc<[Id]>,
    survey: Scan,
}

impl Delta {
    pub fn build(
        facts: &Facts,
        states: &Store<State>,
        previous: Option<(&Facts, &Store<State>)>,
        scope: Option<&[Id]>,
    ) -> Self {
        let Some(previous) = previous else {
            let added = facts
                .entities()
                .iter()
                .map(|entity| entity.id)
                .collect::<Arc<_>>();
            return Self {
                changed: added.clone(),
                added,
                moved: Arc::from([]),
                resized: Arc::from([]),
                overflow: Arc::from([]),
                clipped: Arc::from([]),
                pending: Arc::from([]),
                retired: Arc::from([]),
                survey: Scan::default(),
            };
        };
        compare((facts, states), previous, scope)
    }

    pub fn changed(&self) -> &[Id] {
        &self.changed
    }

    pub fn retired(&self) -> &[Id] {
        &self.retired
    }

    pub fn added(&self) -> &[Id] {
        &self.added
    }

    pub fn moved(&self) -> &[Id] {
        &self.moved
    }

    pub fn resized(&self) -> &[Id] {
        &self.resized
    }

    pub fn overflow(&self) -> &[Id] {
        &self.overflow
    }

    pub fn clipped(&self) -> &[Id] {
        &self.clipped
    }

    pub fn pending(&self) -> &[Id] {
        &self.pending
    }

    pub const fn survey(&self) -> Scan {
        self.survey
    }
}

fn compare(
    current: (&Facts, &Store<State>),
    previous: (&Facts, &Store<State>),
    scope: Option<&[Id]>,
) -> Delta {
    let (facts, states) = current;
    let (oldfacts, oldstates) = previous;
    let mut added = Vec::new();
    let mut moved = Vec::new();
    let mut resized = Vec::new();
    let mut overflow = Vec::new();
    let mut clipped = Vec::new();
    let mut pending = Vec::new();
    let mut changed = Vec::new();
    {
        let mut visit = |slot: usize| {
            let entity = &facts.entities()[slot];
            let state = &states[slot];
            let id = entity.id;
            let Some(old) = oldfacts.slot(id).and_then(|slot| oldstates.get(slot)) else {
                added.push(id);
                changed.push(id);
                return;
            };
            let mut revised = push(
                &mut pending,
                id,
                match (state, old) {
                    (State::Resolved(_), State::Resolved(_))
                    | (State::Deferred(_), State::Deferred(_)) => false,
                    (State::Blocked(a), State::Blocked(b)) => a != b,
                    _ => true,
                },
            );
            if let (Some(metric), Some(old)) = (state.metric(), old.metric()) {
                revised |= push(&mut moved, id, metric.origin() != old.origin());
                revised |= push(&mut resized, id, metric.size() != old.size());
                revised |= push(&mut overflow, id, metric.overflow() != old.overflow());
                revised |= push(&mut clipped, id, metric.clipped() != old.clipped());
            }
            if revised {
                changed.push(id);
            }
        };
        if let Some(scope) = scope {
            for id in scope {
                visit(facts.slot(*id).expect("planned geometry identity"));
            }
        } else {
            for slot in 0..states.count() {
                visit(slot);
            }
        }
    }
    changed.sort_unstable();
    let mut retired = scope.map_or_else(
        || {
            oldfacts
                .entities()
                .iter()
                .filter_map(|entity| facts.slot(entity.id).is_none().then_some(entity.id))
                .collect::<Vec<_>>()
        },
        |_| Vec::new(),
    );
    retired.sort_unstable();
    Delta {
        changed: changed.into(),
        added: added.into(),
        moved: moved.into(),
        resized: resized.into(),
        overflow: overflow.into(),
        clipped: clipped.into(),
        pending: pending.into(),
        retired: retired.into(),
        survey: Scan {
            compared: scope.map_or(states.count(), |ids| ids.len()),
            scanned: scope.map_or(oldfacts.entities().len(), |_| 0),
            indexed: 0,
        },
    }
}

fn push(target: &mut Vec<Id>, id: Id, changed: bool) -> bool {
    if changed {
        target.push(id);
    }
    changed
}
