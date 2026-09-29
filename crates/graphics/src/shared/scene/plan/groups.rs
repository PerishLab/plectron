use super::{Builder, Layer};
use crate::shared::{Fault, Frame, Item, Mapping};
use std::sync::Arc;

impl Builder<'_> {
    pub(super) fn finish(&self, frame: &Frame) -> Result<(), Fault> {
        let remaining = self.mappings.len();
        if remaining == 0 {
            return Ok(());
        }
        Err(Fault(format!(
            "scene traversal left {remaining} of {} frame mappings",
            frame.mappings().len()
        )))
    }

    pub(super) fn excluded(
        &mut self,
        items: &Arc<[Item]>,
        mapping: &Mapping,
    ) -> Result<bool, Fault> {
        let Some(preparation) = self.preparation.as_deref_mut() else {
            return Ok(false);
        };
        preparation.subtree(
            items,
            mapping,
            (self.destination, self.spaces.descendants(mapping.id)),
        )
    }
    pub(super) fn group(
        &mut self,
        items: &Arc<[Item]>,
        mapping: &Mapping,
        opacity: u8,
    ) -> Result<Layer, Fault> {
        if !self.grouping {
            return self.layer(items, mapping, opacity);
        }
        if let Some(preparation) = self.preparation.as_deref_mut()
            && let Some(charge) = preparation.prune(items, mapping)
        {
            self.charge(charge.rectangles)?;
            self.samples = self.samples.saturating_add(charge.samples);
            self.texels = self.texels.saturating_add(charge.texels);
            if self.samples > 1_000_000_000 || self.texels > 16_777_216 {
                return Err(Fault("retained group work budget exhausted".into()));
            }
            if charge.mappings > 0 {
                self.mappings
                    .nth(charge.mappings - 1)
                    .expect("retained group mappings");
            }
            return Ok(Layer {
                opacity,
                bounds: None,
                primitives: Vec::new(),
            });
        }
        let mark = self
            .preparation
            .as_deref()
            .map_or(0, crate::shared::Preparation::mark);
        let rectangles = self.rectangles;
        let samples = self.samples;
        let texels = self.texels;
        let mappings = self.mappings.len();
        let layer = self.layer(items, mapping, opacity)?;
        if layer.bounds.is_none()
            && opacity != 0
            && let Some(preparation) = self.preparation.as_deref_mut()
        {
            preparation.empty(
                items,
                mapping,
                (
                    crate::shared::preparation::Charge {
                        rectangles: self.rectangles - rectangles,
                        samples: self.samples - samples,
                        texels: self.texels - texels,
                        mappings: mappings - self.mappings.len(),
                    },
                    mark,
                ),
            );
        }
        Ok(layer)
    }
}

#[derive(PartialEq, Eq)]
pub(super) enum Demand {
    None,
    Pixels,
}
impl From<u8> for Demand {
    fn from(opacity: u8) -> Self {
        if opacity == 0 {
            Self::None
        } else {
            Self::Pixels
        }
    }
}
impl Builder<'_> {
    pub(super) fn omit(&mut self, space: u64) {
        let descendants = self.spaces.descendants(space);
        if descendants > 0 {
            self.mappings
                .nth(descendants - 1)
                .expect("undemanded group mappings");
        }
        if let Some(preparation) = self.preparation.as_deref_mut() {
            preparation.omit(descendants + 1);
        }
    }
}
