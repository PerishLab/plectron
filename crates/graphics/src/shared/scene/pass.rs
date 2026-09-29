use crate::shared::{Extent, Fault, Frame, Layer, Output};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Pass(Arc<Content>);

#[derive(Debug)]
struct Content {
    frame: u64,
    output: Output,
    layer: Layer,
}
impl Pass {
    pub fn new(frame: &Frame, output: Output) -> Result<Self, Fault> {
        Self::build(frame, output, None)
    }
    pub(crate) fn build(
        frame: &Frame,
        output: Output,
        preparation: Option<&mut crate::shared::Preparation>,
    ) -> Result<Self, Fault> {
        if frame.extent() != output.logical() {
            return Err(Fault("frame differs from target logical extent".into()));
        }
        Ok(Self(Arc::new(Content {
            frame: frame.id(),
            output,
            layer: super::plan::build(frame, output, preparation)?,
        })))
    }
    pub fn id(&self) -> u64 {
        self.0.frame
    }
    pub fn output(&self) -> Output {
        self.0.output
    }
    pub fn extent(&self) -> Extent {
        self.0.output.pixels()
    }
    pub fn layer(&self) -> &Layer {
        &self.0.layer
    }
}
