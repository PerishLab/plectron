use super::presentation::Observation;
use crate::page::Candidate;
use crate::{Display, Fault};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Applied,
    Obsolete,
    Deferred,
    Blocked,
}

impl Disposition {
    pub fn applied(self) -> bool {
        self == Self::Applied
    }
}

impl Display {
    pub(crate) fn project(&mut self, candidate: Candidate) -> Result<(), Fault> {
        if self.state() == crate::State::Closed {
            return Err(Fault("display closed".into()));
        }
        let Candidate {
            layout,
            scene,
            scrolls,
            watching,
            replay,
        } = candidate;
        self.watching = watching.iter().copied().collect();
        self.policies(&scrolls, Some(&layout), Some(&scene), &replay)?;
        self.layout = Some(layout);
        self.scene = Some(scene);
        self.effect = None;
        Ok(())
    }
    pub fn scroll(&mut self, delta: i64) -> Result<(), Fault> {
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| Fault("scene absent".into()))?
            .id();
        self.pan(scene, 0, delta)
    }
    pub fn pan(&mut self, scene: u64, space: u64, delta: i64) -> Result<(), Fault> {
        self.moveby(scene, space, delta, super::ScrollCause::Program)
    }
    fn moveby(
        &mut self,
        scene: u64,
        space: u64,
        delta: i64,
        cause: super::ScrollCause,
    ) -> Result<(), Fault> {
        self.check()?;
        if let Some(fault) = &self.presentation.fault {
            return Err(Fault(fault.clone()));
        }
        let current = self
            .scene
            .as_ref()
            .ok_or_else(|| Fault("scene absent".into()))?;
        if current.id() != scene {
            return Err(Fault("visual update refers to stale scene".into()));
        }
        let maximum = current
            .range(space, self.visual.extent)
            .ok_or_else(|| Fault("space is not scrollable".into()))?;
        let offset = (self.visual.offset(space) as i128 + i128::from(delta))
            .clamp(0, maximum as i128) as usize;
        let scene = current.clone();
        let layout = self.layout.clone();
        let mut offsets = self.visual.offsets.clone();
        offsets.insert(space, offset);
        let reasons = BTreeMap::from([(space, cause)]);
        if self.offsets(
            offsets,
            super::change::Context {
                reasons: &reasons,
                scene: Some(&scene),
                layout: layout.as_deref(),
                replay: &[],
            },
        )? {
            self.refresh()?;
        }
        Ok(())
    }
    pub(crate) fn policies(
        &mut self,
        policies: &[super::Policy],
        layout: Option<&domengine::Layout>,
        next: Option<&graphics::shared::Scene>,
        watching: &[domengine::Node],
    ) -> Result<bool, Fault> {
        let Some(next) = next else {
            self.scrolls.clear();
            return self.offsets(
                BTreeMap::new(),
                super::change::Context {
                    reasons: &BTreeMap::new(),
                    scene: None,
                    layout,
                    replay: watching,
                },
            );
        };
        let previous = std::mem::take(&mut self.scrolls);
        let mut spaces = std::collections::BTreeSet::new();
        for policy in policies {
            let space = layout
                .and_then(|layout| super::control::Geometry::new(layout).scroller(policy.node));
            if let Some(space) = space
                && !spaces.insert(space)
            {
                return Err(Fault("scroll space has multiple policy owners".into()));
            }
            let active = previous
                .get(&policy.lease)
                .filter(|control| control.node == policy.node && control.version == policy.version)
                .is_none_or(|control| control.active);
            self.scrolls.insert(
                policy.lease,
                super::Control {
                    node: policy.node,
                    version: policy.version,
                    space,
                    action: policy.action,
                    active,
                },
            );
        }
        let mut offsets = self.visual.offsets.clone();
        let mut reasons = BTreeMap::new();
        if let (Some(before), Some(after)) = (self.layout.as_deref(), layout) {
            for (lease, control) in self.scrolls.iter().filter(|(_, control)| !control.active) {
                let Some(prior) = previous.get(lease) else {
                    continue;
                };
                let (Some(oldspace), Some(newspace)) = (prior.space, control.space) else {
                    continue;
                };
                let offset = offsets.get(&oldspace).copied().unwrap_or(0);
                let Some(anchored) = super::control::anchor(
                    before,
                    after,
                    super::control::Anchor {
                        node: control.node,
                        offset,
                    },
                ) else {
                    continue;
                };
                let Some(range) = next.range(newspace, self.visual.extent) else {
                    continue;
                };
                let anchored = anchored.min(range);
                if anchored != offset {
                    offsets.remove(&oldspace);
                    offsets.insert(newspace, anchored);
                    reasons.insert(newspace, super::ScrollCause::Anchor);
                }
            }
        }
        for control in self.scrolls.values_mut().filter(|control| control.active) {
            if let Some((space, offset, cause)) =
                super::control::activate(control, layout, next, self.visual.extent)
            {
                offsets.insert(space, offset);
                reasons.insert(space, cause);
            }
        }
        offsets.retain(|space, offset| {
            let Some(range) = next.range(*space, self.visual.extent) else {
                return false;
            };
            *offset = (*offset).min(range);
            true
        });
        self.offsets(
            offsets,
            super::change::Context {
                reasons: &reasons,
                scene: Some(next),
                layout,
                replay: watching,
            },
        )
    }
    pub fn wheel(&mut self, x: usize, y: usize, delta: i64) -> Result<Disposition, Fault> {
        self.check()?;
        let Some(view) = &self.presentation.visible else {
            return Ok(Disposition::Deferred);
        };
        let observed = view.scroller(x, y);
        self.reconcile(observed, delta)
    }
    fn reconcile(
        &mut self,
        observed: Observation<Option<u64>>,
        delta: i64,
    ) -> Result<Disposition, Fault> {
        let Some(space) = observed.value else {
            return Ok(Disposition::Blocked);
        };
        let Some(scene) = &self.scene else {
            return Ok(Disposition::Deferred);
        };
        if scene.range(space, self.visual.extent).is_none() {
            return Ok(Disposition::Obsolete);
        }
        let scene = scene.id();
        let revision = self.visual.revision;
        self.interact(space, delta);
        self.moveby(scene, space, delta, super::ScrollCause::User)?;
        Ok(if self.visual.revision == revision {
            Disposition::Blocked
        } else {
            Disposition::Applied
        })
    }
    fn interact(&mut self, space: u64, delta: i64) {
        if delta == 0 {
            return;
        }
        let Some(maximum) = self
            .scene
            .as_ref()
            .and_then(|scene| scene.range(space, self.visual.extent))
        else {
            return;
        };
        let offset = (self.visual.offset(space) as i128 + i128::from(delta))
            .clamp(0, maximum as i128) as usize;
        let active = delta > 0 && offset == maximum;
        for control in self
            .scrolls
            .values_mut()
            .filter(|control| control.space == Some(space))
        {
            if matches!(control.action, super::Action::Follow(_)) {
                control.active = active;
            }
        }
    }
}
