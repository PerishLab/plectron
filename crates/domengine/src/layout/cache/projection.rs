use crate::layout::{
    fragment::{builder::Builder, tree::Branch},
    measure::View,
};
use crate::{Fault, Node};
use serde::Deserialize;
use shared::cache::{Cache, Cost, Manager};
use std::sync::{Arc, Weak};

type Position = (Option<Node>, [usize; 2]);
const SPACES: usize = 8;
#[derive(Clone)]
struct Entry {
    branch: Weak<Branch>,
    position: Position,
}
pub(in crate::layout) struct Projection {
    pub payloads: crate::layout::text::paint::Cache,
    cache: Cache<Node, Entry>,
    config: config::Store,
    setting: Option<config::Value>,
    query: Option<config::Value>,
    spaces: usize,
    analysis: bool,
    pub materialization: bool,
    pub reused: usize,
    pub work: super::report::Construction,
    pub relocated: usize,
}
impl Projection {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            payloads: crate::layout::text::paint::Cache::new(manager, config.clone())?,
            cache: manager.register("fragments", super::admission::POLICY)?,
            config,
            setting: None,
            query: None,
            spaces: SPACES,
            analysis: false,
            materialization: true,
            reused: 0,
            work: Default::default(),
            relocated: 0,
        })
    }
    pub fn temporary() -> Self {
        Self::new(&Manager::default(), config::Store::default()).unwrap()
    }
    pub fn maintain(&mut self) {
        self.payloads.maintain();
        self.cache.maintain();
    }
    pub fn clear(&mut self) {
        self.payloads.clear();
        self.cache.clear();
    }
    pub fn close(&mut self) {
        self.payloads.close();
        self.cache.close();
    }
    pub fn enable(&mut self, enabled: bool) {
        self.payloads.enable(enabled);
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.payloads.configure()?;
        self.materialization = self
            .config
            .get("layout.materialization")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.materialization: {error}")))?
            .unwrap_or(true);
        self.analysis = self
            .config
            .get("layout.analysis")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.analysis: {error}")))?
            .unwrap_or(false);
        let query = self.config.get("layout.query.spaces");
        if query != self.query {
            self.spaces = query
                .clone()
                .map(usize::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config layout.query.spaces: {error}")))?
                .unwrap_or(SPACES);
            self.cache.clear();
            self.query = query;
        }
        self.reused = 0;
        self.work = Default::default();
        self.relocated = 0;
        let setting = self.config.get("layout.fragments");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(shared::cache::Policy::deserialize)
                .transpose()
                .map_err(|e| Fault(format!("config layout.fragments: {e}")))?
                .unwrap_or(super::admission::POLICY);
            self.cache
                .configure(policy)
                .map_err(|e| Fault(e.to_string()))?;
            self.setting = setting;
        }
        Ok(())
    }
    pub fn publish(&mut self) {
        self.payloads.publish();
        self.cache.publish();
    }
    pub fn build(
        &mut self,
        measured: &View<'_>,
        position: Position,
        prior: Option<&Arc<Branch>>,
    ) -> Result<Arc<Branch>, Fault> {
        self.work.requests += 1;
        let key = measured.material().node();
        let previous = self.cache.lookup(&key).and_then(|entry| {
            entry
                .branch
                .upgrade()
                .map(|branch| (branch, entry.position))
        });
        if let Some((branch, placement)) = &previous
            && branch.owner.matches(measured.material())
        {
            let same = measured.matches(&branch.basis);
            if !same && self.analysis {
                self.work.analysis.compare(&branch.basis, measured);
            }
            if same {
                self.reused += branch.count;
                if *placement != position {
                    self.relocated += branch.count;
                }
                if *placement != position {
                    self.retain(branch, position);
                }
                return Ok(branch.clone());
            }
        }
        let query = self.spaces;
        let reference = previous
            .as_ref()
            .map(|(branch, _)| branch.clone())
            .or_else(|| prior.cloned());
        let mut builder = Builder {
            placements: Vec::new(),
            links: Vec::new(),
            projection: self,
            owner: key,
            previous: reference,
            query,
        };
        let root = measured.relative([0; 2]);
        builder.project(
            &root,
            [0; 2],
            (
                None,
                crate::Rect {
                    x: 0,
                    y: 0,
                    width: super::super::constraints::LIMIT,
                    height: super::super::constraints::LIMIT,
                },
            ),
        )?;
        let branch = builder.finish(measured)?;
        self.work.branches += 1;
        self.work.fragments += branch.fragments.len();
        self.retain(&branch, position);
        Ok(branch)
    }
    fn retain(&mut self, branch: &Arc<Branch>, position: Position) {
        let entry = Entry {
            branch: Arc::downgrade(branch),
            position,
        };
        let cost = Cost::new(std::mem::size_of::<Entry>());
        if let Err((entry, cost)) = self.cache.replace(&branch.owner.node(), entry, cost) {
            self.cache.store(branch.owner.node(), entry, cost);
        }
    }
}
