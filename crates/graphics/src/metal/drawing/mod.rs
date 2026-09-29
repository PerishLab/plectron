pub(crate) mod coverage;
mod pipeline;
mod vertices;
use crate::shared::{Extent, Fault, Layer, Pass, Primitive, Rect};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::*;
pub use pipeline::{Pipelines, pipeline};
use std::ptr::NonNull;
use vertices::draw;
type Device = ProtocolObject<dyn MTLDevice>;
type Texture = ProtocolObject<dyn MTLTexture>;
type Command = ProtocolObject<dyn MTLCommandBuffer>;
struct Attachment {
    texture: Retained<Texture>,
    region: Rect,
}
pub fn encode(
    hardware: (&Device, &Pipelines),
    frame: &Pass,
    output: (&Texture, &Command),
    store: &mut crate::metal::residency::Store,
) -> Result<(), Fault> {
    let plan = frame.layer();
    Recorder {
        device: hardware.0,
        pipelines: hardware.1,
        store,
    }
    .layer(
        (
            plan,
            Rect {
                x: 0,
                y: 0,
                width: frame.extent().width,
                height: frame.extent().height,
            },
        ),
        output,
        true,
    )
}
struct Recorder<'a> {
    device: &'a Device,
    pipelines: &'a Pipelines,
    store: &'a mut crate::metal::residency::Store,
}
impl<'a> Recorder<'a> {
    fn layer(
        &mut self,
        data: (&Layer, Rect),
        output: (&Texture, &Command),
        root: bool,
    ) -> Result<(), Fault> {
        let mut textures = Vec::new();
        for primitive in &data.0.primitives {
            if let Primitive::Group(group) = primitive {
                textures.push(self.group(group, output.1)?);
            }
        }
        let descriptor = MTLRenderPassDescriptor::new();
        // SAFETY: Attachment zero exists in every render pass descriptor.
        let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(output.0));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        let channel = if root { 1.0 } else { 0.0 };
        attachment.setClearColor(MTLClearColor {
            red: channel,
            green: channel,
            blue: channel,
            alpha: channel,
        });
        let encoder = output
            .1
            .renderCommandEncoderWithDescriptor(&descriptor)
            .ok_or_else(|| Fault("render encoder allocation failed".into()))?;
        let origin = [data.1.x as f32, data.1.y as f32];
        // SAFETY: The fixed path/image shaders read two copied floats for the physical render-target origin.
        unsafe {
            encoder.setFragmentBytes_length_atIndex(NonNull::from(&origin).cast(), 8, 3);
        }
        let result = self.contents((&encoder, data.1), &data.0.primitives, &textures);
        encoder.endEncoding();
        result
    }
    fn contents(
        &mut self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        primitives: &[Primitive],
        textures: &[Option<Attachment>],
    ) -> Result<(), Fault> {
        let mut index = 0;
        let mut rectangles = Vec::new();
        let mut paths = crate::metal::buffer::Batch::default();
        for primitive in primitives {
            match primitive {
                Primitive::Coverage(coverage, color) => {
                    self.flush(output, &mut rectangles)?;
                    self.paths(output, &mut paths)?;
                    self.coverage(output, coverage, *color)?;
                }
                Primitive::Image(sample) => {
                    self.flush(output, &mut rectangles)?;
                    self.paths(output, &mut paths)?;
                    self.image(output, sample)?;
                }
                Primitive::Fill(rect, color) => {
                    self.paths(output, &mut paths)?;
                    rectangles.push((*rect, *color));
                }
                Primitive::Path(coverage, color) => {
                    self.flush(output, &mut rectangles)?;
                    self.path(output, (coverage, *color), &mut paths)?;
                }
                Primitive::Group(group) => {
                    self.flush(output, &mut rectangles)?;
                    self.paths(output, &mut paths)?;
                    self.composite(output, textures[index].as_ref(), group.opacity)?;
                    index += 1;
                }
            }
        }
        self.paths(output, &mut paths)?;
        self.flush(output, &mut rectangles)
    }
    fn image(
        &mut self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        sample: &crate::shared::Sample,
    ) -> Result<(), Fault> {
        let texture = self.store.texture(self.device, &sample.raster)?;
        output.0.setRenderPipelineState(&self.pipelines.image);
        let mut settings = [0.0; 8];
        settings[..4].copy_from_slice(&sample.target);
        settings[4..].copy_from_slice(&sample.clip.edges());
        // SAFETY: The fixed image shader reads eight copied floats and the command retains its bound texture.
        unsafe {
            output.0.setFragmentTexture_atIndex(Some(&texture), 0);
            output.0.setFragmentBytes_length_atIndex(
                NonNull::from(&settings).cast(),
                std::mem::size_of_val(&settings),
                1,
            );
        }
        draw(
            self.device,
            output.0,
            &crate::metal::vertices::encode(output.1, &[(sample.bounds(), 0)]),
        )
    }
    fn path<'b>(
        &mut self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        entry: crate::metal::buffer::Entry<'b>,
        paths: &mut crate::metal::buffer::Batch<'b>,
    ) -> Result<(), Fault> {
        if let Some(reason) = paths.boundary(entry.0, self.store.limit) {
            match reason {
                crate::metal::buffer::Split::Evaluation => self.store.batches.switched += 1,
                crate::metal::buffer::Split::Overlap => self.store.batches.overlaps += 1,
                crate::metal::buffer::Split::Limit => self.store.batches.limited += 1,
            }
            self.paths(output, paths)?;
        }
        paths.push(entry);
        Ok(())
    }
    fn paths(
        &mut self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        paths: &mut crate::metal::buffer::Batch<'_>,
    ) -> Result<(), Fault> {
        if paths.entries.is_empty() {
            return Ok(());
        }
        output
            .0
            .setRenderPipelineState(match paths.entries[0].0.evaluation() {
                crate::shared::Evaluation::Linear => &self.pipelines.path,
                crate::shared::Evaluation::Quadratic => &self.pipelines.curve,
            });
        let path = self.store.path(self.device, &paths.entries)?;
        crate::metal::buffer::draw(output.0, &path, output.1, &paths.entries);
        self.store.batches.paths += paths.entries.len() as u64;
        self.store.batches.calls += 1;
        paths.clear();
        Ok(())
    }
    fn flush(
        &mut self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        rectangles: &mut Vec<(Rect, u32)>,
    ) -> Result<(), Fault> {
        if rectangles.is_empty() {
            return Ok(());
        }
        output.0.setRenderPipelineState(&self.pipelines.solid);
        draw(
            self.device,
            output.0,
            &crate::metal::vertices::encode(output.1, rectangles),
        )?;
        rectangles.clear();
        Ok(())
    }
}

impl Recorder<'_> {
    fn group(&mut self, group: &Layer, command: &Command) -> Result<Option<Attachment>, Fault> {
        let Some(region) = group.bounds else {
            return Ok(None);
        };
        let extent = Extent {
            width: region.width,
            height: region.height,
        };
        let texture = self.store.pool.acquire(self.device, extent)?;
        self.layer((group, region), (&texture, command), false)?;
        Ok(Some(Attachment { texture, region }))
    }
    fn composite(
        &self,
        output: (&ProtocolObject<dyn MTLRenderCommandEncoder>, Rect),
        attachment: Option<&Attachment>,
        opacity: u8,
    ) -> Result<(), Fault> {
        let Some(attachment) = attachment else {
            return Ok(());
        };
        output.0.setRenderPipelineState(&self.pipelines.composite);
        let offset = [
            output.1.x as f32 - attachment.region.x as f32,
            output.1.y as f32 - attachment.region.y as f32,
        ];
        // SAFETY: The command retains its source texture and copies two floats mapping parent pixels to child-local pixels.
        unsafe {
            output
                .0
                .setFragmentTexture_atIndex(Some(&attachment.texture), 0);
            output
                .0
                .setFragmentBytes_length_atIndex(NonNull::from(&offset).cast(), 8, 1);
        }
        draw(
            self.device,
            output.0,
            &crate::metal::vertices::encode(
                output.1,
                &[(attachment.region, u32::from(opacity) * 0x010101)],
            ),
        )
    }
}
