use crate::shared::{Coverage, Fault, Rect, coverage::Mask};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLBuffer, MTLDevice, MTLPrimitiveType, MTLRenderCommandEncoder, MTLResourceOptions,
};
use std::ptr::NonNull;

#[derive(Clone)]
pub(crate) struct Buffer {
    pub buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    pub count: usize,
}
pub(crate) fn upload(
    device: &ProtocolObject<dyn MTLDevice>,
    coverage: &Coverage,
) -> Result<Buffer, Fault> {
    let count = coverage.strips().len();
    let base = count * 16;
    let mut words = Vec::with_capacity(count * 4 + coverage.samples().len().div_ceil(4));
    for strip in coverage.strips() {
        let offset = match &strip.mask {
            Mask::Solid => 0,
            Mask::Samples(range) => base + range.start + 1,
        };
        words.extend([
            strip.x as u32,
            strip.y as u32,
            strip.width as u32,
            offset as u32,
        ]);
    }
    for chunk in coverage.samples().chunks(4) {
        let mut bytes = [0; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        words.push(u32::from_le_bytes(bytes));
    }
    // SAFETY: Metal copies the live u32 slice; bounded descriptors address packed sample bytes in this immutable buffer.
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(words.as_ptr().cast_mut().cast()).expect("nonempty coverage"),
            std::mem::size_of_val(words.as_slice()),
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or_else(|| Fault("coverage buffer allocation failed".into()))?;
    Ok(Buffer { buffer, count })
}
impl super::Recorder<'_> {
    pub(super) fn coverage(
        &mut self,
        output: (
            &ProtocolObject<dyn objc2_metal::MTLRenderCommandEncoder>,
            Rect,
        ),
        placement: &crate::shared::preparation::Placement,
        color: u32,
    ) -> Result<(), Fault> {
        let coverage = &placement.coverage;
        if coverage.strips().is_empty() {
            return Ok(());
        }
        let buffer = self.store.coverage(self.device, coverage)?;
        output.0.setRenderPipelineState(&self.pipelines.coverage);
        let settings = [
            output.1.width as f32,
            output.1.height as f32,
            output.1.x as f32 - placement.offset[0] as f32,
            output.1.y as f32 - placement.offset[1] as f32,
            ((color >> 16) & 255) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
            0.0,
        ];
        // SAFETY: One instance addresses one admitted descriptor; the command retains the immutable buffer and copies eight settings floats.
        unsafe {
            output
                .0
                .setVertexBuffer_offset_atIndex(Some(&buffer.buffer), 0, 0);
            output
                .0
                .setFragmentBuffer_offset_atIndex(Some(&buffer.buffer), 0, 0);
            output.0.setVertexBytes_length_atIndex(
                NonNull::from(&settings).cast(),
                std::mem::size_of_val(&settings),
                1,
            );
            output.0.setFragmentBytes_length_atIndex(
                NonNull::from(&settings).cast(),
                std::mem::size_of_val(&settings),
                1,
            );
            output
                .0
                .drawPrimitives_vertexStart_vertexCount_instanceCount(
                    MTLPrimitiveType::Triangle,
                    0,
                    6,
                    buffer.count,
                );
        }
        Ok(())
    }
}
