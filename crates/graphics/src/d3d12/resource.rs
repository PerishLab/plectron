use crate::shared::Fault;
use shared::Resource;

pub fn shader() -> Result<&'static str, Fault> {
    Resource::new(include_bytes!("../../resources/draw.hlsl"))
        .text()
        .map_err(crate::d3d12::error)
}
