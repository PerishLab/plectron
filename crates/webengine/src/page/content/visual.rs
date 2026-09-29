use super::Content;
use crate::Fault;

impl Content {
    pub(super) fn observe(&mut self) -> Result<bool, Fault> {
        let visualizing = self
            .runtime
            .as_mut()
            .is_some_and(|runtime| runtime.addons().visualizing());
        if !visualizing {
            return Ok(false);
        }
        let (Some(layout), Some(scene), Some(runtime)) = (
            self.layout.clone(),
            self.scene.clone(),
            self.runtime.as_mut(),
        ) else {
            return Ok(false);
        };
        let mut visual = self.visual.clone();
        visual.offsets.retain(|space, offset| {
            if let Some(range) = scene.range(*space, visual.extent) {
                *offset = (*offset).min(range);
                true
            } else {
                false
            }
        });
        let frame = graphics::shared::Frame::new(scene, visual).map_err(crate::error)?;
        let before = runtime.addons().document.revision();
        runtime.host(|engine, web| web.visualize(engine, &layout, &frame))?;
        Ok(runtime.addons().document.revision() != before)
    }

    pub(in crate::page) fn converge(&mut self, mut changed: bool) -> Result<bool, Fault> {
        for _ in 0..=32 {
            if !self.observe()? {
                return Ok(changed);
            }
            changed |= self.build()?;
            changed |= self.settle()?;
        }
        Err(Fault("visual observation did not converge".into()))
    }
}
