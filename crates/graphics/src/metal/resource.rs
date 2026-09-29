use crate::shared::Fault;
use shared::Resource;

pub fn shader() -> Result<&'static str, Fault> {
    Resource::new(include_bytes!("../../resources/draw.metal"))
        .text()
        .map_err(crate::metal::error)
}
