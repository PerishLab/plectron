use crate::shared::Fault;
use objc2::runtime::ProtocolObject;
use objc2_metal::*;
use std::ptr::NonNull;
type Device = ProtocolObject<dyn MTLDevice>;

pub(super) fn draw(
    device: &Device,
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    vertices: &[f32],
) -> Result<(), Fault> {
    let pointer = NonNull::new(vertices.as_ptr().cast_mut().cast()).expect("nonempty vertices");
    if std::mem::size_of_val(vertices) <= 4096 {
        // SAFETY: Metal copies this live f32 slice into command-owned storage within its 4 KiB inline limit.
        unsafe {
            encoder.setVertexBytes_length_atIndex(pointer, std::mem::size_of_val(vertices), 0);
        }
    } else {
        // SAFETY: Metal copies exactly the live f32 slice and the command retains the resulting immutable buffer.
        let buffer = unsafe {
            device.newBufferWithBytes_length_options(
                pointer,
                std::mem::size_of_val(vertices),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .ok_or_else(|| Fault("vertex buffer allocation failed".into()))?;
        // SAFETY: Vertex storage contains five floats per vertex at offset zero and is retained by this command.
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
        }
    }
    // SAFETY: The fixed shader reads five floats per vertex; the draw count stays within the copied storage.
    unsafe {
        encoder.drawPrimitives_vertexStart_vertexCount(
            MTLPrimitiveType::Triangle,
            0,
            vertices.len() / 5,
        );
    }
    Ok(())
}
