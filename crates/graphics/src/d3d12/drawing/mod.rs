pub(crate) mod coverage;
pub(crate) mod frame;
mod pipeline;
pub(crate) mod vertices;

use crate::d3d12::buffer::{Batch, Split, transition};
use crate::d3d12::ffi::{Com, constants, tables, types};
use crate::d3d12::memory::Texture;
use crate::shared::{Extent, Fault, Layer, Pass, Primitive, Rect};
pub(crate) use frame::Frame;
pub use pipeline::{Pipelines, pipeline};
use vertices::draw;

struct Attachment {
    texture: Texture,
    region: Rect,
}

pub(crate) fn encode(
    hardware: (&Com, &Pipelines),
    pass: &Pass,
    output: (&Com, &mut Frame),
    store: &mut crate::d3d12::residency::Store,
) -> Result<(), Fault> {
    let region = Rect {
        x: 0,
        y: 0,
        width: pass.extent().width,
        height: pass.extent().height,
    };
    let mut recorder = Recorder {
        device: hardware.0,
        pipelines: hardware.1,
        frame: output.1,
        store,
    };
    recorder.layer((pass.layer(), region), output.0, true)?;
    transition(
        &recorder.frame.list,
        output.0,
        constants::TARGET,
        constants::COPYSOURCE,
    );
    Ok(())
}

struct Recorder<'a> {
    device: &'a Com,
    pipelines: &'a Pipelines,
    frame: &'a mut Frame,
    store: &'a mut crate::d3d12::residency::Store,
}

impl Recorder<'_> {
    fn layer(&mut self, data: (&Layer, Rect), target: &Com, root: bool) -> Result<(), Fault> {
        let mut textures = Vec::new();
        for primitive in &data.0.primitives {
            if let Primitive::Group(group) = primitive {
                textures.push(self.group(group)?);
            }
        }
        let rtv = self.frame.rtv(self.device, target)?;
        let channel = if root { 1.0f32 } else { 0.0f32 };
        let clear = [channel; 4];
        let viewport = types::Viewport {
            x: 0.0,
            y: 0.0,
            width: data.1.width as f32,
            height: data.1.height as f32,
            near: 0.0,
            far: 1.0,
        };
        let scissor = types::Scissor {
            left: 0,
            top: 0,
            right: data.1.width as i32,
            bottom: data.1.height as i32,
        };
        let origin = [data.1.x as f32, data.1.y as f32];
        let heaps = [self.frame.srvheap.raw()];
        // SAFETY: list is open; rtv, srvheap, and root signature remain retained on frame.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.targets)(self.frame.list.raw(), 1, &rtv, 0, std::ptr::null());
            (table.clearrtv)(
                self.frame.list.raw(),
                rtv,
                clear.as_ptr(),
                0,
                std::ptr::null(),
            );
            (table.viewports)(self.frame.list.raw(), 1, &viewport);
            (table.scissors)(self.frame.list.raw(), 1, &scissor);
            (table.heaps)(self.frame.list.raw(), 1, heaps.as_ptr());
            (table.root)(self.frame.list.raw(), self.pipelines.root.raw());
            (table.topology)(self.frame.list.raw(), constants::LISTTRIANGLE);
            (table.constants)(self.frame.list.raw(), 0, 2, origin.as_ptr().cast(), 8);
        }
        let result = self.contents(data.1, &data.0.primitives, &textures);
        for item in textures.into_iter().flatten() {
            transition(
                &self.frame.list,
                &item.texture.resource,
                constants::PIXEL,
                constants::TARGET,
            );
        }
        result
    }

    fn contents(
        &mut self,
        region: Rect,
        primitives: &[Primitive],
        textures: &[Option<Attachment>],
    ) -> Result<(), Fault> {
        let mut index = 0;
        let mut rectangles = Vec::new();
        let mut paths = Batch::default();
        for primitive in primitives {
            match primitive {
                Primitive::Coverage(coverage, color) => {
                    self.flush(region, &mut rectangles)?;
                    self.paths(region, &mut paths)?;
                    self.coverage(region, coverage, *color)?;
                }
                Primitive::Image(sample) => {
                    self.flush(region, &mut rectangles)?;
                    self.paths(region, &mut paths)?;
                    self.image(region, sample)?;
                }
                Primitive::Fill(rect, color) => {
                    self.paths(region, &mut paths)?;
                    rectangles.push((*rect, *color));
                }
                Primitive::Path(coverage, color) => {
                    self.flush(region, &mut rectangles)?;
                    self.path(region, (coverage, *color), &mut paths)?;
                }
                Primitive::Group(group) => {
                    self.flush(region, &mut rectangles)?;
                    self.paths(region, &mut paths)?;
                    self.composite(region, textures[index].as_ref(), group.opacity)?;
                    index += 1;
                }
            }
        }
        self.paths(region, &mut paths)?;
        self.flush(region, &mut rectangles)
    }

    fn image(&mut self, region: Rect, sample: &crate::shared::Sample) -> Result<(), Fault> {
        let texture = self.store.texture(
            (self.device, &self.frame.list),
            &mut self.frame.arena,
            &sample.raster,
        )?;
        self.frame.retained.push(texture.resource.clone());
        let gpu = self.frame.srv(self.device, &texture.resource)?;
        let mut settings = [0.0f32; 8];
        settings[..4].copy_from_slice(&sample.target);
        settings[4..].copy_from_slice(&sample.clip.edges());
        // SAFETY: Image PSO reads 8 settings floats at b0[0..8] and Texture2D at root table 3; texture is retained.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.pipeline)(self.frame.list.raw(), self.pipelines.image.raw());
            (table.constants)(self.frame.list.raw(), 0, 8, settings.as_ptr().cast(), 0);
            (table.table)(self.frame.list.raw(), 3, gpu);
        }
        draw(
            &self.frame.list,
            &mut self.frame.arena,
            &vertices::encode(region, &[(sample.bounds(), 0)]),
        )
    }

    fn path<'b>(
        &mut self,
        region: Rect,
        entry: crate::d3d12::buffer::Entry<'b>,
        paths: &mut Batch<'b>,
    ) -> Result<(), Fault> {
        if let Some(reason) = paths.boundary(entry.0, self.store.limit) {
            match reason {
                Split::Evaluation => self.store.batches.switched += 1,
                Split::Overlap => self.store.batches.overlaps += 1,
                Split::Limit => self.store.batches.limited += 1,
            }
            self.paths(region, paths)?;
        }
        paths.push(entry);
        Ok(())
    }

    fn paths(&mut self, region: Rect, paths: &mut Batch<'_>) -> Result<(), Fault> {
        if paths.entries.is_empty() {
            return Ok(());
        }
        let state = match paths.entries[0].0.evaluation() {
            crate::shared::Evaluation::Linear => &self.pipelines.path,
            crate::shared::Evaluation::Quadratic => &self.pipelines.curve,
        };
        let path = self.store.path(
            (self.device, &self.frame.list),
            &mut self.frame.arena,
            &paths.entries,
        )?;
        self.frame.retained.push(path.resource.clone());
        // SAFETY: state is a live PSO belonging to the active root signature.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.pipeline)(self.frame.list.raw(), state.raw());
        }
        crate::d3d12::buffer::draw(
            &self.frame.list,
            &mut self.frame.arena,
            &path,
            (region, &paths.entries),
        )?;
        self.store.batches.paths += paths.entries.len() as u64;
        self.store.batches.calls += 1;
        paths.clear();
        Ok(())
    }

    fn flush(&mut self, region: Rect, rectangles: &mut Vec<(Rect, u32)>) -> Result<(), Fault> {
        if rectangles.is_empty() {
            return Ok(());
        }
        // SAFETY: solid PSO belongs to the active root signature.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.pipeline)(self.frame.list.raw(), self.pipelines.solid.raw());
        }
        draw(
            &self.frame.list,
            &mut self.frame.arena,
            &vertices::encode(region, rectangles),
        )?;
        rectangles.clear();
        Ok(())
    }

    fn group(&mut self, group: &Layer) -> Result<Option<Attachment>, Fault> {
        let Some(region) = group.bounds else {
            return Ok(None);
        };
        let extent = Extent {
            width: region.width,
            height: region.height,
        };
        let texture = self.store.pool.acquire(self.device, extent)?;
        self.frame.retained.push(texture.resource.clone());
        self.layer((group, region), &texture.resource, false)?;
        transition(
            &self.frame.list,
            &texture.resource,
            constants::TARGET,
            constants::PIXEL,
        );
        Ok(Some(Attachment { texture, region }))
    }

    fn composite(
        &mut self,
        region: Rect,
        attachment: Option<&Attachment>,
        opacity: u8,
    ) -> Result<(), Fault> {
        let Some(attachment) = attachment else {
            return Ok(());
        };
        let gpu = self.frame.srv(self.device, &attachment.texture.resource)?;
        let offset = [
            region.x as f32 - attachment.region.x as f32,
            region.y as f32 - attachment.region.y as f32,
        ];
        // SAFETY: Composite PSO reads offset at b0[0..2] and child Texture2D at root table 3; texture is retained.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.pipeline)(self.frame.list.raw(), self.pipelines.composite.raw());
            (table.constants)(self.frame.list.raw(), 0, 2, offset.as_ptr().cast(), 0);
            (table.table)(self.frame.list.raw(), 3, gpu);
        }
        draw(
            &self.frame.list,
            &mut self.frame.arena,
            &vertices::encode(
                region,
                &[(attachment.region, u32::from(opacity) * 0x010101)],
            ),
        )
    }
}
