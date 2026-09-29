use crate::{Fault, error};
use graphics::shared::{Extent, Raster};
use std::io::Cursor;

pub(super) fn decode(bytes: &[u8]) -> Result<Raster, Fault> {
    if bytes.len() > super::Kind::Image.limit() {
        return Err(Fault("encoded image budget exhausted".into()));
    }
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: 16_777_216 });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(error)?;
    let info = reader.info();
    if info.animation_control.is_some()
        || info.icc_profile.is_some()
        || info.bit_depth == png::BitDepth::Sixteen
    {
        return Err(Fault(
            "animated, ICC-profiled and 16-bit PNGs are outside this profile".into(),
        ));
    }
    let extent = Extent {
        width: info.width as usize,
        height: info.height as usize,
    };
    if extent.area().map_err(error)? > 1_048_576 {
        return Err(Fault("decoded image pixel budget exhausted".into()));
    }
    let length = reader
        .output_buffer_size()
        .filter(|length| *length <= 4_194_304)
        .ok_or_else(|| Fault("decoded image byte budget exhausted".into()))?;
    let mut bytes = vec![0; length];
    let output = reader.next_frame(&mut bytes).map_err(error)?;
    reader.finish().map_err(error)?;
    let mut rgba = Vec::with_capacity(extent.area().map_err(error)? * 4);
    for pixel in bytes[..output.buffer_size()].chunks_exact(output.color_type.samples()) {
        match output.color_type {
            png::ColorType::Rgb => rgba.extend([pixel[0], pixel[1], pixel[2], 255]),
            png::ColorType::Rgba => rgba.extend_from_slice(pixel),
            png::ColorType::Grayscale => rgba.extend([pixel[0], pixel[0], pixel[0], 255]),
            png::ColorType::GrayscaleAlpha => rgba.extend([pixel[0], pixel[0], pixel[0], pixel[1]]),
            png::ColorType::Indexed => return Err(Fault("PNG palette was not expanded".into())),
        }
    }
    Raster::rgba(extent, &rgba).map_err(error)
}
