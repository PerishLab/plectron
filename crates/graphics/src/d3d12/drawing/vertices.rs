use crate::d3d12::buffer::{Arena, Entry};
use crate::d3d12::ffi::{Com, tables};
use crate::shared::{Fault, Rect};

pub(super) fn draw(list: &Com, arena: &mut Arena, vertices: &[f32]) -> Result<(), Fault> {
    let length = std::mem::size_of_val(vertices);
    // SAFETY: vertices is a live contiguous [f32] slice viewed as immutable bytes.
    let bytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr().cast::<u8>(), length) };
    let (_, _, address) = arena.write(bytes, 4)?;
    // SAFETY: Root param 1 binds 5-float vertices at t0; vertex count matches vertices.len() / 5.
    unsafe {
        let table = list.vtable::<tables::List>();
        (table.srv)(list.raw(), 1, address);
        (table.draw)(list.raw(), (vertices.len() / 5) as u32, 1, 0, 0);
    }
    Ok(())
}

pub(super) fn encode(extent: Rect, rectangles: &[(Rect, u32)]) -> Vec<f32> {
    let mut output = Vec::new();
    for (rect, color) in rectangles {
        let left = (rect.x as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0;
        let right =
            ((rect.x + rect.width) as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (rect.y as f32 - extent.y as f32) / extent.height as f32 * 2.0;
        let bottom =
            1.0 - ((rect.y + rect.height) as f32 - extent.y as f32) / extent.height as f32 * 2.0;
        let channels = [
            ((color >> 16) & 255) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
        ];
        for point in [
            [left, top],
            [left, bottom],
            [right, top],
            [right, top],
            [left, bottom],
            [right, bottom],
        ] {
            output.extend_from_slice(&[point[0], point[1], channels[0], channels[1], channels[2]]);
        }
    }
    output
}

pub(crate) fn placements(extent: Rect, entries: &[Entry<'_>]) -> Vec<f32> {
    let mut output = Vec::with_capacity(entries.len() * 8);
    for (coverage, color) in entries {
        let rect = coverage.bounds;
        output.extend_from_slice(&[
            (rect.x as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0,
            1.0 - (rect.y as f32 - extent.y as f32) / extent.height as f32 * 2.0,
            ((rect.x + rect.width) as f32 - extent.x as f32) / extent.width as f32 * 2.0 - 1.0,
            1.0 - ((rect.y + rect.height) as f32 - extent.y as f32) / extent.height as f32 * 2.0,
            ((color >> 16) & 255) as f32 / 255.0,
            ((color >> 8) & 255) as f32 / 255.0,
            (color & 255) as f32 / 255.0,
            0.0,
        ]);
    }
    output
}
