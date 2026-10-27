use super::{Error, error};
use crate::shared::Image;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, Drawable, Gcontext, Rectangle};

pub(super) struct Area {
    pub(super) drawable: Drawable,
    pub(super) gc: Gcontext,
    pub(super) width: u16,
    pub(super) height: u16,
}
pub(super) fn clear(
    conn: &x11rb::rust_connection::RustConnection,
    area: Area,
) -> Result<(), Error> {
    conn.poly_fill_rectangle(
        area.drawable,
        area.gc,
        &[Rectangle {
            x: 0,
            y: 0,
            width: area.width,
            height: area.height,
        }],
    )
    .map_err(error)?
    .check()
    .map_err(error)
}
pub(super) struct Put<'a> {
    pub(super) drawable: Drawable,
    pub(super) gc: Gcontext,
    pub(super) image: &'a Image,
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) swap: bool,
}
pub(super) fn put(
    conn: &x11rb::rust_connection::RustConnection,
    put: Put<'_>,
) -> Result<(), Error> {
    let mut data = Vec::with_capacity(usize::from(put.width) * usize::from(put.height) * 4);
    for y in 0..usize::from(put.height) {
        let source = y * put.image.extent.width;
        for x in 0..usize::from(put.width) {
            let value = put.image.pixels[source + x];
            let value = if put.swap { value.swap_bytes() } else { value };
            data.extend_from_slice(&value.to_ne_bytes());
        }
    }
    let order = if conn.setup().image_byte_order == x11rb::protocol::xproto::ImageOrder::LSB_FIRST {
        x11rb::image::ImageOrder::LsbFirst
    } else {
        x11rb::image::ImageOrder::MsbFirst
    };
    let image = x11rb::image::Image::new(
        put.width,
        put.height,
        x11rb::image::ScanlinePad::Pad32,
        24,
        x11rb::image::BitsPerPixel::B32,
        order,
        std::borrow::Cow::Owned(data),
    )
    .map_err(error)?;
    image.put(conn, put.drawable, put.gc, 0, 0).map_err(error)?;
    Ok(())
}
