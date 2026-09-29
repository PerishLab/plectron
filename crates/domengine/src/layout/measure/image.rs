use super::super::{
    Constraints, Content, Fragment, Shaper,
    input::Entry,
    measure::{Measure, Measured, View},
};
use crate::layout::cache::Measurement;
use crate::{Fault, Rect};

impl<S: Shaper> Measure<'_, S> {
    pub(in crate::layout) fn image<'a>(
        &self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Measured<'a> {
        let intrinsic = element.key.image.expect("image dimensions");
        let style = &element.key.style;
        let known = [
            constraints.known[0].or(style.width),
            constraints.known[1].or(style.height),
        ];
        let ratio = intrinsic.width as f64 / intrinsic.height as f64;
        let width = known[0].unwrap_or_else(|| {
            known[1].map_or(intrinsic.width, |height| {
                (height as f64 * ratio).round() as usize
            })
        });
        let width = style.bound(0, width);
        let height = style.bound(
            1,
            known[1].unwrap_or_else(|| (width as f64 / ratio).round() as usize),
        );
        let width = if known[0].is_none() {
            style.bound(0, (height as f64 * ratio).round() as usize)
        } else {
            width
        };
        Measured::new(
            element,
            Measurement {
                work: 1,
                pending: false,
                size: [width, height],
                overflow: [width, height],
                clipped: true,
                children: Vec::new(),
                text: None,
                quality: geoengine::Quality::Measured,
            },
        )
    }
}
impl crate::layout::fragment::builder::Builder<'_> {
    pub(in crate::layout) fn picture(
        &mut self,
        measured: &View<'_>,
        rect: Rect,
        parent: usize,
    ) -> Result<(), Fault> {
        let material = measured.material();
        self.push(Fragment {
            id: 0,
            space: self.placements[parent].local.space + 1,
            parent: Some(parent),
            node: Some(material.node().id()),
            target: Some(material.target()),
            rect,
            clip: self.placements[parent].clip.intersect(rect),
            content: Content::Image {
                raster: material.picture().unwrap().clone(),
            },
        })?;
        Ok(())
    }
}
