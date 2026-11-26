use crate::d3d12::buffer::Arena;
use crate::d3d12::ffi::{Com, tables};
use crate::shared::Fault;

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
