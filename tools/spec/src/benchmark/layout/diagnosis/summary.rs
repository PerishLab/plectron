use domengine::{Activity, Category, Event};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const LIMIT: usize = 16;

#[derive(Default, Serialize)]
pub(super) struct Summary {
    events: usize,
    families: usize,
    variants: usize,
    variantfamilies: usize,
    requests: usize,
    retains: usize,
    hits: usize,
    windows: usize,
    carries: usize,
    misses: usize,
    cold: usize,
    dependencies: usize,
    budgets: usize,
    disabled: usize,
    computes: usize,
    bypasses: usize,
    screens: usize,
    stores: usize,
    attributedwork: usize,
    reusedwork: usize,
    retainedwork: usize,
    windowwork: usize,
    carrywork: usize,
    repeatedwork: usize,
    intervalwork: usize,
}

#[derive(Serialize)]
pub(super) struct Cause {
    fingerprint: u64,
    request: u64,
    category: Category,
    identities: usize,
    requests: usize,
    hits: usize,
    misses: usize,
    cold: usize,
    dependencies: usize,
    budgets: usize,
    bypasses: usize,
    disabled: usize,
    computes: usize,
    screens: usize,
    stores: usize,
    attributedwork: usize,
    repeatedwork: usize,
    intervalwork: usize,
    weight: usize,
}

struct Group {
    category: Category,
    identities: BTreeSet<u64>,
    requests: usize,
    hits: usize,
    misses: usize,
    cold: usize,
    dependencies: usize,
    budgets: usize,
    bypasses: usize,
    disabled: usize,
    computes: usize,
    screens: usize,
    stores: usize,
    attributedwork: usize,
    repeatedwork: usize,
    firstwork: Option<usize>,
    weight: usize,
}

pub(super) fn compile(events: &[Event]) -> (Summary, Vec<Cause>) {
    collect(std::iter::once(events))
}

pub(super) fn aggregate(stages: &[Vec<Event>]) -> (Summary, Vec<Cause>) {
    collect(stages.iter().map(Vec::as_slice))
}

fn collect<'a>(stages: impl Iterator<Item = &'a [Event]>) -> (Summary, Vec<Cause>) {
    let stages = stages.collect::<Vec<_>>();
    let mut summary = Summary {
        events: stages.iter().map(|events| events.len()).sum(),
        ..Default::default()
    };
    let mut groups = BTreeMap::new();
    let mut families: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    for events in stages {
        let mut computed = BTreeSet::new();
        for event in events {
            let key = (event.fingerprint, event.request);
            families.entry(key.0).or_default().insert(key.1);
            let repeated = event.activity == Activity::Compute && !computed.insert(key);
            let group = groups.entry(key).or_insert_with(|| Group::new(event));
            group.observe(event, repeated);
            summary.observe(event);
        }
    }
    summary.families = families.len();
    summary.variants = groups.len();
    summary.variantfamilies = families
        .values()
        .filter(|requests| requests.len() > 1)
        .count();
    let mut causes = groups
        .into_iter()
        .map(|((fingerprint, request), group)| group.cause(fingerprint, request))
        .filter(Cause::interesting)
        .collect::<Vec<_>>();
    summary.repeatedwork = causes.iter().map(|cause| cause.repeatedwork).sum();
    summary.intervalwork = causes.iter().map(|cause| cause.intervalwork).sum();
    causes.sort_by(|left, right| {
        right
            .repeatedwork
            .cmp(&left.repeatedwork)
            .then(right.intervalwork.cmp(&left.intervalwork))
            .then(right.screens.cmp(&left.screens))
            .then(right.attributedwork.cmp(&left.attributedwork))
    });
    causes.truncate(LIMIT);
    (summary, causes)
}

impl Summary {
    fn observe(&mut self, event: &Event) {
        match event.activity {
            Activity::Request => self.requests += 1,
            Activity::Retain => {
                self.retains += 1;
                self.retainedwork += event.work;
            }
            Activity::Hit => {
                self.hits += 1;
                self.reusedwork += event.work;
            }
            Activity::Window => {
                self.windows += 1;
                self.windowwork += event.work;
            }
            Activity::Carry => {
                self.carries += 1;
                self.carrywork += event.work;
            }
            Activity::Miss => self.misses += 1,
            Activity::Cold => self.cold += 1,
            Activity::Dependency => self.dependencies += 1,
            Activity::Budget => self.budgets += 1,
            Activity::Disabled => self.disabled += 1,
            Activity::Compute => {
                self.computes += 1;
                self.attributedwork += event.work;
            }
            Activity::Bypass => self.bypasses += 1,
            Activity::Screen => self.screens += 1,
            Activity::Store => self.stores += 1,
        }
    }
}

impl Cause {
    fn interesting(&self) -> bool {
        [
            self.repeatedwork,
            self.intervalwork,
            self.screens,
            self.dependencies,
            self.budgets,
            self.bypasses,
            self.disabled,
        ]
        .into_iter()
        .any(|value| value > 0)
    }
}

impl Group {
    fn new(event: &Event) -> Self {
        Self {
            category: event.category,
            identities: Default::default(),
            requests: 0,
            hits: 0,
            misses: 0,
            cold: 0,
            dependencies: 0,
            budgets: 0,
            bypasses: 0,
            disabled: 0,
            computes: 0,
            screens: 0,
            stores: 0,
            attributedwork: 0,
            repeatedwork: 0,
            firstwork: None,
            weight: 0,
        }
    }

    fn observe(&mut self, event: &Event, repeated: bool) {
        self.identities.insert(event.identity);
        self.weight = self.weight.max(event.weight);
        match event.activity {
            Activity::Request => self.requests += 1,
            Activity::Hit => self.hits += 1,
            Activity::Miss => self.misses += 1,
            Activity::Cold => self.cold += 1,
            Activity::Dependency => self.dependencies += 1,
            Activity::Budget => self.budgets += 1,
            Activity::Bypass => self.bypasses += 1,
            Activity::Disabled => self.disabled += 1,
            Activity::Compute => {
                self.computes += 1;
                self.attributedwork += event.work;
                if repeated {
                    self.repeatedwork += event.work;
                }
                self.firstwork.get_or_insert(event.work);
            }
            Activity::Screen => self.screens += 1,
            Activity::Store => self.stores += 1,
            _ => {}
        }
    }

    fn cause(self, fingerprint: u64, request: u64) -> Cause {
        let intervalwork = self
            .attributedwork
            .saturating_sub(self.firstwork.unwrap_or_default());
        Cause {
            fingerprint,
            request,
            category: self.category,
            identities: self.identities.len(),
            requests: self.requests,
            hits: self.hits,
            misses: self.misses,
            cold: self.cold,
            dependencies: self.dependencies,
            budgets: self.budgets,
            bypasses: self.bypasses,
            disabled: self.disabled,
            computes: self.computes,
            screens: self.screens,
            stores: self.stores,
            attributedwork: self.attributedwork,
            repeatedwork: self.repeatedwork,
            intervalwork,
            weight: self.weight,
        }
    }
}
