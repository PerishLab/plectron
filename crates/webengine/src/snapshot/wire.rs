use crate::{Fault, Snapshot, error};
use graphics::shared::wire::{Font, Item, Manifest, Resources};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

pub type View = Snapshot<domengine::Fragment<Font, u64>, Item>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub snapshot: View,
    pub resources: Manifest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub version: u32,
    pub snapshot: View,
    pub resources: Resources,
}
impl Bundle {
    pub fn new(mut snapshot: Snapshot) -> Self {
        let resources = RefCell::new(Resources::default());
        let fragments = std::mem::take(&mut snapshot.fragments)
            .iter()
            .cloned()
            .map(|fragment| {
                fragment.map(
                    |font| resources.borrow_mut().font(font),
                    |raster| resources.borrow_mut().raster(raster),
                )
            })
            .collect();
        let items = std::mem::take(&mut snapshot.items)
            .into_iter()
            .map(|item| resources.borrow_mut().item(item))
            .collect();
        Self {
            version: 1,
            snapshot: snapshot.map(fragments, items),
            resources: resources.into_inner(),
        }
    }
    pub fn inspect(self) -> Inspection {
        Inspection {
            snapshot: self.snapshot,
            resources: self.resources.manifest(),
        }
    }
    pub fn restore(mut self) -> Result<Snapshot, Fault> {
        if self.version != 1 {
            return Err(Fault("unsupported snapshot bundle version".into()));
        }
        let fragments = std::mem::take(&mut self.snapshot.fragments)
            .iter()
            .cloned()
            .map(|fragment| self.fragment(fragment))
            .collect::<Result<Vec<_>, _>>()?;
        let items = std::mem::take(&mut self.snapshot.items)
            .into_iter()
            .map(|item| self.resources.restore(item).map_err(error))
            .collect::<Result<_, _>>()?;
        Ok(self.snapshot.map(fragments, items))
    }
    fn fragment(
        &self,
        fragment: domengine::Fragment<Font, u64>,
    ) -> Result<domengine::Fragment, Fault> {
        let fragment = fragment.map(
            |font| self.resources.resolve(font),
            |id| self.resources.image(id),
        );
        let font = match &fragment.content {
            domengine::Content::Text { font, .. } => Some(font.clone().map_err(error)?),
            _ => None,
        };
        let raster = match &fragment.content {
            domengine::Content::Image { raster } => Some(raster.clone().map_err(error)?),
            _ => None,
        };
        Ok(fragment.map(
            |_| font.expect("validated font"),
            |_| raster.expect("validated raster"),
        ))
    }
}
