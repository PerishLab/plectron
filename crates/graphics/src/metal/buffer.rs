use crate::shared::{Fault, Geometry, Rect};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLBuffer, MTLDevice, MTLPrimitiveType, MTLRenderCommandEncoder, MTLResourceOptions,
};
use std::ptr::NonNull;

pub(crate) type Entry<'a> = (&'a Geometry, u32);
#[derive(Clone)]
pub(crate) struct Path {
    pub buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
}
#[derive(Default)]
pub(crate) struct Batch<'a> {
    pub entries: Vec<Entry<'a>>,
    bytes: usize,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Batches {
    pub paths: u64,
    pub calls: u64,
    pub overlaps: u64,
    pub limited: u64,
    pub switched: u64,
}
pub(crate) enum Split {
    Evaluation,
    Overlap,
    Limit,
}
impl<'a> Batch<'a> {
    pub fn boundary(&self, coverage: &Geometry, limit: usize) -> Option<Split> {
        if self.entries.is_empty() {
            return None;
        }
        if self.entries[0].0.evaluation() != coverage.evaluation() {
            return Some(Split::Evaluation);
        }
        if self.entries.len() >= limit || self.bytes + size(coverage) > 4_194_304 {
            return Some(Split::Limit);
        }
        self.entries
            .iter()
            .any(|(previous, _)| {
                let intersection = previous.bounds.intersect(coverage.bounds);
                intersection.width != 0 && intersection.height != 0
            })
            .then_some(Split::Overlap)
    }
    pub fn push(&mut self, entry: Entry<'a>) {
        self.bytes += size(entry.0);
        self.entries.push(entry);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}
fn size(coverage: &Geometry) -> usize {
    std::mem::size_of_val(coverage.edges())
        + coverage.curves().len() * 32
        + std::mem::size_of_val(coverage.bands().data())
        + 44
}
pub(crate) fn path(
    device: &ProtocolObject<dyn MTLDevice>,
    entries: &[Entry<'_>],
) -> Result<Path, Fault> {
    let mut words = vec![0; entries.len() * 8];
    for (index, (coverage, _)) in entries.iter().enumerate() {
        words.resize(words.len().next_multiple_of(4), 0);
        let edge = words.len();
        words.extend(
            coverage
                .edges()
                .iter()
                .flatten()
                .map(|value| value.to_bits()),
        );
        words.extend(
            coverage
                .curves()
                .iter()
                .flat_map(|curve| packed(curve).map(f32::to_bits)),
        );
        let band = words.len();
        words.extend_from_slice(coverage.bands().data());
        words[index * 8..index * 8 + 4].copy_from_slice(&[
            edge as u32,
            band as u32,
            coverage.bands().origin() as u32,
            coverage.bands().stride() as u32,
        ]);
        words[index * 8 + 4..index * 8 + 8]
            .copy_from_slice(&coverage.clip().edges().map(f32::to_bits));
    }
    let pointer = NonNull::new(words.as_ptr().cast_mut().cast()).expect("batch header");
    // SAFETY: Metal copies live u32 storage. Bounded headers address 16-byte-aligned float4 edges and packed u32 bands.
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            pointer,
            std::mem::size_of_val(words.as_slice()),
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or_else(|| Fault("path buffer allocation failed".into()))?;
    Ok(Path { buffer })
}
pub(crate) fn draw(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    path: &Path,
    extent: Rect,
    entries: &[Entry<'_>],
) {
    let placements = crate::metal::vertices::placements(extent, entries);
    // SAFETY: At most 128 eight-float placements fit Metal's 4096-byte inline limit. Each instance addresses its own admitted geometry header.
    unsafe {
        encoder.setVertexBytes_length_atIndex(
            NonNull::new(placements.as_ptr().cast_mut().cast()).expect("batch placements"),
            std::mem::size_of_val(placements.as_slice()),
            0,
        );
        encoder.setFragmentBuffer_offset_atIndex(Some(&path.buffer), 0, 0);
        encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
            MTLPrimitiveType::Triangle,
            0,
            6,
            entries.len(),
        );
    }
}

fn packed(curve: &crate::shared::Curve) -> [f32; 8] {
    let [p, q, r] = curve.points;
    [
        p[0],
        p[1],
        q[0],
        q[1],
        r[0],
        r[1],
        curve.direction as f32,
        u8::from(curve.quadratic) as f32,
    ]
}
