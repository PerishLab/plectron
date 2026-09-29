use crate::shared::{Backend, Extent, Fault, Frame, Image, Item, State};

mod compute;
pub use compute::{Compute, Task};

#[derive(Default)]
pub struct Cpu;

impl Backend for Cpu {
    type Work = Image;

    fn submit(&mut self, frame: &crate::shared::Pass) -> Result<Image, Fault> {
        let extent = frame.extent();
        let plan = frame.layer();
        let mut pixels = vec![0xffffffff; extent.area()?];
        paint(plan, extent, &mut pixels);
        let image = Image {
            extent,
            pixels: pixels.into_iter().map(|pixel| pixel & 0xffffff).collect(),
        };
        Ok(image)
    }

    fn poll(&mut self, _: &mut Image) -> Result<State, Fault> {
        Ok(State::Complete)
    }
    fn read(&mut self, frame: &Image) -> Result<Image, Fault> {
        Ok(frame.clone())
    }
}

pub fn raster(extent: Extent, items: &[Item]) -> Result<Image, Fault> {
    let scene = crate::shared::Scene::new(extent, 0, items.to_vec())?;
    Cpu.render(&Frame::new(scene, crate::shared::Visual::new(extent))?)
}

fn paint(layer: &crate::shared::Layer, extent: Extent, pixels: &mut [u32]) {
    for primitive in &layer.primitives {
        match primitive {
            crate::shared::Primitive::Coverage(coverage, color) => {
                strips(coverage, *color, extent, pixels)
            }
            crate::shared::Primitive::Image(sample) => image(sample, extent, pixels),
            crate::shared::Primitive::Fill(rect, color) => fill(*rect, *color, extent, pixels),
            crate::shared::Primitive::Path(coverage, color) => {
                path(coverage, *color, extent, pixels)
            }
            crate::shared::Primitive::Group(group) => composite(group, extent, pixels),
        }
    }
}
fn blend(source: u32, target: u32, opacity: u8) -> u32 {
    let alpha = f64::from(source >> 24) / 255.0 * f64::from(opacity) / 255.0;
    let mut output = 0;
    for shift in [0, 8, 16, 24] {
        let source = f64::from((source >> shift) & 255) * f64::from(opacity) / 255.0;
        let target = f64::from((target >> shift) & 255);
        let channel = (source + target * (1.0 - alpha)).round().clamp(0.0, 255.0) as u32;
        output |= channel << shift;
    }
    output
}

fn fill(rect: crate::shared::Rect, color: u32, extent: Extent, pixels: &mut [u32]) {
    for y in rect.y..rect.y + rect.height {
        pixels[y * extent.width + rect.x..y * extent.width + rect.x + rect.width]
            .fill(0xff000000 | color);
    }
}
fn composite(group: &crate::shared::Layer, extent: Extent, pixels: &mut [u32]) {
    let Some(bounds) = group.bounds else {
        return;
    };
    let mut source = vec![0; pixels.len()];
    paint(group, extent, &mut source);
    for y in bounds.y..bounds.y + bounds.height {
        let start = y * extent.width + bounds.x;
        let end = start + bounds.width;
        for (target, source) in pixels[start..end].iter_mut().zip(&source[start..end]) {
            *target = blend(*source, *target, group.opacity);
        }
    }
}

fn path(coverage: &crate::shared::Geometry, color: u32, extent: Extent, pixels: &mut [u32]) {
    let rect = coverage.bounds;
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let source = colorize(coverage.samples(x, y), color);
            let target = &mut pixels[y * extent.width + x];
            *target = blend(source, *target, 255);
        }
    }
}

fn image(sample: &crate::shared::Sample, extent: Extent, pixels: &mut [u32]) {
    let rect = sample.bounds();
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let target = &mut pixels[y * extent.width + x];
            *target = blend(sample.pixel(x, y), *target, 255);
        }
    }
}

fn colorize(samples: u8, color: u32) -> u32 {
    let alpha = (u32::from(samples) * 255 + 8) / 16;
    let mut source = alpha << 24;
    for shift in [0, 8, 16] {
        source |= ((((color >> shift) & 255) * alpha + 127) / 255) << shift;
    }
    source
}
fn strips(
    placement: &crate::shared::preparation::Placement,
    color: u32,
    extent: Extent,
    pixels: &mut [u32],
) {
    let coverage = &placement.coverage;
    for strip in coverage.strips() {
        let x = strip.x + placement.offset[0];
        let y = strip.y + placement.offset[1];
        let row = &mut pixels[y * extent.width + x..y * extent.width + x + strip.width];
        match &strip.mask {
            crate::shared::coverage::Mask::Solid => row.fill(colorize(16, color)),
            crate::shared::coverage::Mask::Samples(range) => {
                boundary(row, &coverage.samples()[range.clone()], color);
            }
        }
    }
}

fn boundary(row: &mut [u32], samples: &[u8], color: u32) {
    for (target, &count) in row.iter_mut().zip(samples) {
        *target = blend(colorize(count, color), *target, 255);
    }
}
