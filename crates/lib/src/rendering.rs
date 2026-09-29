use crate::contract::{Extent, Fault};
pub use ::platform::shared::Image;

pub trait Renderer {
    fn paint(&mut self, extent: Extent, color: u32) -> Result<Image, Fault>;
}

#[derive(Default)]
pub struct Solid;

impl Renderer for Solid {
    fn paint(&mut self, extent: Extent, color: u32) -> Result<Image, Fault> {
        if color > 0x00ff_ffff {
            return Err(Fault("color must be a 24-bit RGB integer".into()));
        }
        Ok(Image {
            extent,
            pixels: vec![color; extent.area().map_err(|error| Fault(error.to_string()))?],
        })
    }
}
