use crate::{Fault, layout::text::paragraph::Lines};
use geoengine::Attachment;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Default)]
pub(in crate::layout) struct Bindings(BTreeMap<Attachment, Arc<Lines>>);

impl Bindings {
    pub fn bind(&mut self, id: Attachment, lines: Arc<Lines>) -> Result<(), Fault> {
        if let Some(previous) = self.0.insert(id, lines.clone())
            && !Arc::ptr_eq(&previous, &lines)
        {
            return Err(Fault("geometry attachment identity is ambiguous".into()));
        }
        Ok(())
    }

    pub fn get(&self, id: Attachment) -> Option<&Arc<Lines>> {
        self.0.get(&id)
    }

    pub fn pin(
        &mut self,
        geometry: &Arc<crate::layout::cache::Measurement>,
        source: &Self,
    ) -> Result<bool, Fault> {
        if let Some(attachment) = geometry.text {
            let Some(lines) = source.get(attachment) else {
                return Ok(false);
            };
            self.bind(attachment, lines.clone())?;
        }
        for child in &geometry.children {
            if !self.pin(&child.geometry, source)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
