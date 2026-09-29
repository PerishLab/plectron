use crate::{Display, Fault, error};
use pipeline::Graph;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Work {
    pub updates: u64,
    pub builds: u64,
    pub revisions: u64,
    pub spaces: u64,
    pub offsets: u64,
    pub affected: u64,
    pub elapsed: u64,
}

impl Work {
    pub fn since(self, previous: Self) -> Self {
        Self {
            updates: self.updates - previous.updates,
            builds: self.builds - previous.builds,
            revisions: self.revisions - previous.revisions,
            spaces: self.spaces - previous.spaces,
            offsets: self.offsets - previous.offsets,
            affected: self.affected - previous.affected,
            elapsed: self.elapsed - previous.elapsed,
        }
    }
}

impl Display {
    pub(crate) fn refresh(&mut self) -> Result<(), Fault> {
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| Fault("scene absent".into()))?
            .clone();
        let layout = self.layout.clone();
        let mut offsets = self.visual.offsets.clone();
        offsets.retain(|id, offset| {
            let range = scene.range(*id, self.visual.extent);
            if let Some(range) = range {
                *offset = (*offset).min(range);
                true
            } else {
                false
            }
        });
        self.offsets(
            offsets,
            super::change::Context {
                reasons: &BTreeMap::new(),
                scene: Some(&scene),
                layout: layout.as_deref(),
                replay: &[],
            },
        )?;
        if self.presentation.fault.is_none() {
            self.remap(scene)?;
        }
        Ok(())
    }

    fn remap(&mut self, scene: graphics::shared::Scene) -> Result<(), Fault> {
        let previous = self
            .presentation
            .pending
            .as_ref()
            .or_else(|| self.presentation.issued.as_ref().map(|view| &view.frame))
            .or_else(|| self.presentation.visible.as_ref().map(|view| &view.frame));
        let impact = previous.map(|frame| frame.impact(&scene, &self.visual));
        let input = (scene, self.visual.clone());
        let started = std::time::Instant::now();
        let frame = if let Some(previous) = previous {
            self.mapping.revisions = self.mapping.revisions.saturating_add(1);
            pipeline::step(graphics::shared::operation::Remapping { previous }).run(input)
        } else {
            self.mapping.builds = self.mapping.builds.saturating_add(1);
            pipeline::step(graphics::shared::operation::Mapping).run(input)
        }
        .map_err(error)?;
        let retained = previous.is_some_and(|previous| frame.shares(previous));
        let processed = usize::from(frame.projected() && !retained) * frame.spaces();
        self.mapping.updates = self.mapping.updates.saturating_add(1);
        self.mapping.spaces = self.mapping.spaces.saturating_add(processed as u64);
        self.mapping.offsets = self
            .mapping
            .offsets
            .saturating_add(impact.map_or(0, |impact| impact.offsets as u64));
        self.mapping.affected = self.mapping.affected.saturating_add(impact.map_or_else(
            || frame.mappings().len() as u64,
            |impact| impact.affected as u64,
        ));
        self.mapping.elapsed = self
            .mapping
            .elapsed
            .saturating_add(crate::page::elapsed(started));
        self.presentation.pending = Some(frame);
        Ok(())
    }
}
