use super::Control;
use crate::contract::{Fault, Signal};
use crate::platform::{Backend, Host, Platform};
use crate::rendering::Renderer;
use crate::runtime::Runtime;

pub trait Dispatch {
    fn dispatch(&mut self, signal: Signal) -> Result<(), Fault>;
}

impl<H: Host> Dispatch for Runtime<Control<H>> {
    fn dispatch(&mut self, signal: Signal) -> Result<(), Fault> {
        if self.addons().host.input(&signal)? {
            return self.addons().host.finish();
        }
        let arguments = signal
            .event
            .data()
            .map(|text| vec![jsengine::Value::Text(text.into())])
            .unwrap_or_default();
        for callback in self.addons().callbacks(&signal) {
            self.call(callback, arguments.clone())
                .map_err(|failure| self.diagnostic(&failure))?;
        }
        self.checkpoint()?;
        self.addons().host.finish()?;
        Ok(())
    }
}

pub trait Pump {
    fn active(&mut self) -> bool;
    fn poll(&mut self) -> Result<(), Fault>;
}

impl<B: Backend, R: Renderer> Pump for Runtime<Control<Platform<B, R>>> {
    fn active(&mut self) -> bool {
        self.addons().host.active()
    }

    fn poll(&mut self) -> Result<(), Fault> {
        for signal in self.addons().host.poll()? {
            self.dispatch(signal)?;
        }
        Ok(())
    }
}
