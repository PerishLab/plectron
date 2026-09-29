use crate::metal::{error, resource};
use crate::shared::Fault;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
type Device = ProtocolObject<dyn MTLDevice>;
type Pipeline = ProtocolObject<dyn MTLRenderPipelineState>;
pub struct Pipelines {
    pub(super) coverage: Retained<Pipeline>,
    pub(super) solid: Retained<Pipeline>,
    pub(super) composite: Retained<Pipeline>,
    pub(super) curve: Retained<Pipeline>,
    pub(super) path: Retained<Pipeline>,
    pub(super) image: Retained<Pipeline>,
}
pub fn pipeline(device: &Device) -> Result<(Pipelines, crate::metal::Setup), Fault> {
    let start = std::time::Instant::now();
    let library = device
        .newLibraryWithSource_options_error(&NSString::from_str(resource::shader()?), None)
        .map_err(error)?;
    let librarytime = start.elapsed();
    let start = std::time::Instant::now();
    let pipelines = Pipelines {
        coverage: create((device, &library), "fragmentCoverage", true)?,
        solid: create((device, &library), "fragmentMain", false)?,
        composite: create((device, &library), "fragmentComposite", true)?,
        curve: create((device, &library), "fragmentCurve", true)?,
        path: create((device, &library), "fragmentPath", true)?,
        image: create((device, &library), "fragmentImage", true)?,
    };
    Ok((
        pipelines,
        crate::metal::Setup {
            driver: Default::default(),
            library: librarytime,
            states: start.elapsed(),
        },
    ))
}
fn create(
    hardware: (&Device, &ProtocolObject<dyn MTLLibrary>),
    name: &str,
    blend: bool,
) -> Result<Retained<Pipeline>, Fault> {
    let (device, library) = hardware;
    let vertex = library
        .newFunctionWithName(&NSString::from_str(match name {
            "fragmentPath" | "fragmentCurve" => "vertexPath",
            "fragmentCoverage" => "vertexCoverage",
            _ => "vertexMain",
        }))
        .ok_or_else(|| Fault("vertex function absent".into()))?;
    let fragment = library
        .newFunctionWithName(&NSString::from_str(name))
        .ok_or_else(|| Fault("fragment function absent".into()))?;
    let descriptor = MTLRenderPipelineDescriptor::new();
    descriptor.setVertexFunction(Some(&vertex));
    descriptor.setFragmentFunction(Some(&fragment));
    // SAFETY: Attachment zero exists in every render pipeline descriptor.
    let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    attachment.setBlendingEnabled(blend);
    attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
    attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
    attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(error)
}
