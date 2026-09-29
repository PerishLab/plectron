mod groups;
use super::Paint;
use crate::shared::{
    Clip, Fault, Frame, Geometry, Item, Mapping, Output, Path, Preparation, Rect, Sample,
    Transform, preparation::Placement,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct Layer {
    pub opacity: u8,
    pub bounds: Option<Rect>,
    pub primitives: Vec<Primitive>,
}
#[derive(Debug)]
pub enum Primitive {
    Coverage(Placement, u32),
    Image(Sample),
    Fill(Rect, u32),
    Path(Arc<Geometry>, u32),
    Group(Layer),
}

pub fn build(
    frame: &Frame,
    destination: Output,
    preparation: Option<&mut Preparation>,
) -> Result<Layer, Fault> {
    let mut mappings = frame.mappings().iter();
    let root = mappings.next().expect("frame root mapping");
    let grouping = preparation.as_deref().is_some_and(Preparation::grouping);
    let mut builder = Builder {
        grouping,
        spaces: &frame.scene().spaces,
        preparation,
        mappings,
        destination,
        effects: &frame.scene().effects,
        rectangles: 0,
        samples: 0,
        texels: 0,
        scratch: Vec::new(),
    };
    let layer = builder.layer(frame.scene().items(), root, 255)?;
    builder.finish(frame)?;
    if layer.groups().saturating_mul(destination.pixels().area()?) > 16_777_216 {
        return Err(Fault("intermediate pixel budget exhausted".into()));
    }
    Ok(layer)
}
struct Builder<'a> {
    grouping: bool,
    spaces: &'a super::space::Index,
    preparation: Option<&'a mut Preparation>,
    mappings: std::slice::Iter<'a, Mapping>,
    destination: Output,
    effects: &'a std::collections::BTreeMap<u64, u8>,
    rectangles: usize,
    samples: usize,
    texels: usize,
    scratch: Vec<Paint>,
}
impl Builder<'_> {
    fn layer(&mut self, items: &[Item], mapping: &Mapping, opacity: u8) -> Result<Layer, Fault> {
        let mut primitives = Vec::new();
        self.items(items, mapping, &mut primitives)?;
        let bounds = (opacity != 0)
            .then(|| {
                primitives
                    .iter()
                    .filter_map(Primitive::bounds)
                    .reduce(Rect::union)
            })
            .flatten();
        Ok(Layer {
            bounds,
            opacity,
            primitives,
        })
    }
    fn items(
        &mut self,
        items: &[Item],
        mapping: &Mapping,
        primitives: &mut Vec<Primitive>,
    ) -> Result<(), Fault> {
        for item in items {
            if let Item::Group {
                space,
                opacity,
                items,
            } = item
            {
                let opacity = self.effects.get(&space.id).unwrap_or(opacity);
                let child = self.mappings.next().expect("frame group mapping");
                assert_eq!(child.id, space.id, "scene and frame traversal order");
                if groups::Demand::from(*opacity) == groups::Demand::None {
                    self.omit(space.id);
                    continue;
                }
                if self.excluded(items, child)? {
                    self.omit(space.id);
                    continue;
                }
                if *opacity == 255 && !self.grouping {
                    self.items(items, child, primitives)?;
                    continue;
                }
                let layer = self.group(items, child, *opacity)?;
                if layer.bounds.is_none() {
                    continue;
                }
                if *opacity == 255 {
                    primitives.extend(layer.primitives);
                } else {
                    primitives.push(Primitive::Group(layer));
                }
            } else {
                self.paint(item, mapping, primitives)?;
            }
        }
        Ok(())
    }
    fn paint(
        &mut self,
        item: &Item,
        mapping: &Mapping,
        output: &mut Vec<Primitive>,
    ) -> Result<(), Fault> {
        if let Some(preparation) = self.preparation.as_deref_mut()
            && let Some(work) = preparation.exclude(item, mapping, self.destination)?
        {
            self.charge(work)?;
            return Ok(());
        }
        let mut rectangles = std::mem::take(&mut self.scratch);
        item.paint(
            &mut rectangles,
            &mut |font, glyph| match self.preparation.as_deref_mut() {
                Some(preparation) => preparation.outline(font, glyph),
                None => Ok(font.outline(glyph)?),
            },
        )?;
        self.charge(rectangles.len())?;
        for paint in rectangles.drain(..) {
            self.drawing(paint, mapping, output)?;
        }
        self.scratch = rectangles;
        Ok(())
    }
    fn charge(&mut self, rectangles: usize) -> Result<(), Fault> {
        self.rectangles += rectangles;
        if self.rectangles > 1_048_576 {
            return Err(Fault("drawing budget exhausted".into()));
        }
        Ok(())
    }
    fn geometry(
        &mut self,
        geometry: (Arc<Geometry>, [usize; 2]),
        color: u32,
        output: &mut Vec<Primitive>,
    ) -> Result<(), Fault> {
        let bounds = geometry.0.bounds;
        self.samples = self
            .samples
            .saturating_add(usize::try_from(geometry.0.visits()).unwrap_or(usize::MAX));
        if self.samples > 1_000_000_000 {
            return Err(Fault("geometry work budget exhausted".into()));
        }
        if bounds.width > 0 && bounds.height > 0 {
            let primitive = match self.preparation.as_deref_mut() {
                Some(preparation) => preparation.primitive(geometry, color)?,
                None => Primitive::Path(geometry.0, color),
            };
            output.push(primitive);
        }
        Ok(())
    }
    fn image(&mut self, sample: Sample, output: &mut Vec<Primitive>) -> Result<(), Fault> {
        let bounds = sample.bounds();
        if bounds.width == 0 || bounds.height == 0 {
            return Ok(());
        }
        self.texels = self.texels.saturating_add(sample.raster.extent().area()?);
        self.samples = self.samples.saturating_add(
            bounds
                .width
                .saturating_mul(bounds.height)
                .saturating_mul(16),
        );
        if self.texels > 16_777_216 || self.samples > 1_000_000_000 {
            return Err(Fault("image work budget exhausted".into()));
        }
        output.push(Primitive::Image(sample));
        Ok(())
    }
    fn fill(&mut self, rect: Rect, color: u32, output: &mut Vec<Primitive>) -> Result<(), Fault> {
        let clip = self.destination.clip(rect);
        if clip.integral() {
            output.push(Primitive::Fill(clip.bounds(), color));
            return Ok(());
        }
        let geometry = Arc::new(Geometry::rectangle(clip)?);
        self.geometry((geometry, [0; 2]), color, output)
    }
    fn path(
        &mut self,
        source: (Path, Transform, Clip),
        color: u32,
        output: &mut Vec<Primitive>,
    ) -> Result<(), Fault> {
        let (path, transform, clip) = source;
        let geometry = match self.preparation.as_deref_mut() {
            Some(preparation) => {
                if !preparation.visible(&path, transform, clip)? {
                    return Ok(());
                }
                preparation.path(path, transform, clip)?
            }
            None => (Arc::new(Geometry::clipped(&path, transform, clip)?), [0; 2]),
        };
        self.geometry(geometry, color, output)
    }
    fn drawing(
        &mut self,
        paint: Paint,
        mapping: &Mapping,
        output: &mut Vec<Primitive>,
    ) -> Result<(), Fault> {
        match paint {
            Paint::Image(rect, raster, clip) => {
                if rect.width == 0 || rect.height == 0 {
                    return Ok(());
                }
                let scale = self.destination.scale() as f32;
                let target = [
                    (rect.x + i128::from(mapping.translation[0])) as f32 * scale,
                    (rect.y + i128::from(mapping.translation[1])) as f32 * scale,
                    rect.width as f32 * scale,
                    rect.height as f32 * scale,
                ];
                let clip = self
                    .destination
                    .clip(clip.map_or(mapping.clip, |clip| mapping.project(clip)))
                    .intersect(Clip::rectangle([
                        target[0],
                        target[1],
                        target[0] + target[2],
                        target[1] + target[3],
                    ]));
                let sample = Sample {
                    raster,
                    target,
                    clip,
                };
                self.image(sample, output)?;
            }
            Paint::Fill(rect, color) => {
                let rect = rect.map(mapping.translation).intersect(mapping.clip);
                if rect.width > 0 && rect.height > 0 {
                    self.fill(rect, color, output)?;
                }
            }
            Paint::Path(path, transform, clip, color) => {
                let clip = clip.map_or(mapping.clip, |clip| mapping.project(clip));
                let transform = transform.translate(mapping.translation.map(Into::into));
                let transform = self.destination.transform(transform);
                let clip = self.destination.clip(clip);
                self.path((path, transform, clip), color, output)?;
            }
        }

        Ok(())
    }
}
impl Primitive {
    pub fn bounds(&self) -> Option<Rect> {
        let rect = match self {
            Self::Fill(rect, _) => *rect,
            Self::Path(geometry, _) => geometry.bounds,
            Self::Coverage(coverage, _) => coverage.bounds(),
            Self::Image(sample) => sample.bounds(),
            Self::Group(layer) => return layer.bounds,
        };
        (rect.width > 0 && rect.height > 0).then_some(rect)
    }
}
impl Layer {
    pub fn groups(&self) -> usize {
        self.primitives
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Group(layer) => Some(1 + layer.groups()),
                _ => None,
            })
            .sum()
    }
}
