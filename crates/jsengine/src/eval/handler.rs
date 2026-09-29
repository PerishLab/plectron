use super::{Engine, Fault, Flow, Host, Value};
use crate::syntax::Handler;

impl Engine {
    pub(super) fn handler(
        &mut self,
        handler: &Handler,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Fault> {
        let child = self.scope(scope);
        let mut flow = self.block(&handler.body, child, host)?;
        if let Flow::Throw(value) = &flow
            && let Some(catch) = &handler.catch
        {
            let binding = self.scope(scope);
            if let Some(name) = &catch.name {
                self.define(binding, name.clone(), value.clone(), true)?;
            }
            let child = self.scope(binding);
            flow = self.block(&catch.body, child, host)?;
        }
        if let Some(body) = &handler.finally {
            let child = self.scope(scope);
            let finalization = self.block(body, child, host)?;
            if !matches!(finalization, Flow::Next(_)) {
                flow = finalization;
            }
        }
        Ok(flow.fill(Some(Value::Undefined)))
    }
}
