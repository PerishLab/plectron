use super::super::{Native, Web};
use crate::Fault;
use jsengine::{Engine, Value};

impl Web {
    pub(in crate::addons) fn network(
        &mut self,
        engine: &mut Engine,
    ) -> Result<(Value, Value), Fault> {
        let fetch = self.bind(engine, Native::Fetch(0));
        let scope = self.bind(engine, Native::Requests);
        let requests = engine.object([("scope".into(), scope)].into())?;
        engine.freeze(&requests)?;
        Ok((fetch, requests))
    }
    pub(in crate::addons) fn requesting(
        &mut self,
        native: Native,
        arguments: &[Value],
        engine: &mut Engine,
    ) -> Result<Value, Fault> {
        match (native, arguments) {
            (Native::Fetch(scope), [Value::Text(source)]) => {
                self.fetch.start(scope, source, engine)
            }
            (Native::Requests, []) => {
                if self.fetch.scopes >= 4096 {
                    return Err(Fault("request scope budget exhausted".into()));
                }
                self.fetch.scopes += 1;
                let scope = self.fetch.scopes;
                let fetch = self.bind(engine, Native::Fetch(scope));
                let cancel = self.bind(engine, Native::Cancel(scope));
                let value =
                    engine.object([("fetch".into(), fetch), ("cancel".into(), cancel)].into())?;
                engine.freeze(&value)?;
                Ok(value)
            }
            (Native::Cancel(scope), []) => {
                self.fetch.closed.insert(scope);
                let ids: Vec<_> = self
                    .fetch
                    .pending
                    .iter()
                    .filter(|(_, pending)| pending.scope == scope)
                    .map(|(id, _)| *id)
                    .collect();
                for id in ids {
                    let pending = self.fetch.pending.remove(&id).expect("scope request");
                    pending.cancellation.cancel();
                    self.fetch.report.cancelled += 1;
                    let error = engine
                        .exception(jsengine::Error::Error, Some("request cancelled".into()))?;
                    engine
                        .call(pending.deferred.reject, vec![error], self)
                        .map_err(|failure| self.diagnostics.diagnostic(engine, &failure))?;
                }
                Ok(Value::Undefined)
            }
            (Native::Body(id, format), []) => self.consume(id, format, engine),
            _ => Err(Fault("invalid request arguments".into())),
        }
    }
}
