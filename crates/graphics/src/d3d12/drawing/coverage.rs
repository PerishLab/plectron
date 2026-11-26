use crate::d3d12::buffer::{Arena, stage};
use crate::d3d12::ffi::{Com, constants, tables};
use crate::shared::{Coverage, Fault, Rect, coverage::Mask};

#[derive(Clone)]
pub(crate) struct Buffer {
    pub resource: Com,
    pub address: u64,
    pub count: usize,
    pub bytes: usize,
    pub allocated: usize,
}

pub(crate) fn upload(
    device: &Com,
    list: &Com,
    arena: &mut Arena,
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
    let length = std::mem::size_of_val(words.as_slice());
    // SAFETY: words is an initialized contiguous [u32] slice viewed as immutable bytes.
    let raw = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), length) };
    let (resource, address, allocated) = stage(device, list, arena, (raw, constants::SHADER))?;
    Ok(Buffer {
        resource,
        address,
        count,
        bytes: length,
        allocated,
    })
}

impl super::Recorder<'_> {
    pub(super) fn coverage(
        &mut self,
        region: Rect,
        placement: &crate::shared::preparation::Placement,
        color: u32,
    ) -> Result<(), Fault> {
        let coverage = &placement.coverage;
        if coverage.strips().is_empty() {
            return Ok(());
        }
        let buffer = self.store.coverage(
            (self.device, &self.frame.list),
            &mut self.frame.arena,
            coverage,
        )?;
        self.frame.retained.push(buffer.resource.clone());
        let settings = [
            region.width as f32,
            region.height as f32,
            region.x as f32 - placement.offset[0] as f32,
            region.y as f32 - placement.offset[1] as f32,
            ((color >> 16) & 255) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
            0.0,
        ];
        // SAFETY: Coverage PSO reads eight settings floats at b0[0..8] and strip/sample SRVs at t0/t1; buffer is retained.
        unsafe {
            let table = self.frame.list.vtable::<tables::List>();
            (table.pipeline)(self.frame.list.raw(), self.pipelines.coverage.raw());
            (table.constants)(self.frame.list.raw(), 0, 8, settings.as_ptr().cast(), 0);
            (table.srv)(self.frame.list.raw(), 1, buffer.address);
            (table.srv)(self.frame.list.raw(), 2, buffer.address);
            (table.draw)(self.frame.list.raw(), 6, buffer.count as u32, 0, 0);
        }
        Ok(())
    }
}
