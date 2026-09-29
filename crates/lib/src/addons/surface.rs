use super::convert::integer;
use crate::contract::Extent;
use crate::rendering::{Image, Renderer, Solid};
use crate::runtime::{Addons, Capabilities};
use jsengine::{Engine, Fault, Value};
use std::collections::BTreeMap;

pub struct Surface {
    extent: Extent,
    frame: Option<Image>,
}

impl Surface {
    pub fn new(extent: Extent) -> Result<Self, Fault> {
        extent.area().map_err(|error| Fault(error.to_string()))?;
        Ok(Self {
            extent,
            frame: None,
        })
    }

    pub fn take(&mut self) -> Option<Image> {
        self.frame.take()
    }
}

impl Addons for Surface {
    fn install(
        &mut self,
        engine: &mut Engine,
        capabilities: &mut Capabilities,
    ) -> Result<(), Fault> {
        let fill = engine.native(0);
        let surface = engine.object(BTreeMap::from([
            ("fill".into(), fill),
            ("width".into(), Value::Number(self.extent.width as f64)),
            ("height".into(), Value::Number(self.extent.height as f64)),
        ]))?;
        engine.freeze(&surface)?;
        capabilities.insert("surface".into(), surface);
        Ok(())
    }
}

impl jsengine::Addons for Surface {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, jsengine::Failure> {
        Ok(self.invoke(engine, binding, invocation.arguments)?)
    }
}

impl Surface {
    fn invoke(
        &mut self,
        _: &mut Engine,
        binding: usize,
        arguments: Vec<Value>,
    ) -> Result<Value, Fault> {
        let (0, [value]) = (binding, arguments.as_slice()) else {
            return Err(Fault("invalid surface call".into()));
        };
        let color = integer(value, 0x00ff_ffff)? as u32;
        self.frame = Some(Solid.paint(self.extent, color)?);
        Ok(Value::Undefined)
    }
}
