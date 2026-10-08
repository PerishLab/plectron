use super::{
    super::{Native, Web},
    Body,
};
use crate::{Fault, resource::Response};
use jsengine::{Engine, Value};
use std::collections::BTreeMap;

impl Web {
    pub(super) fn response(
        &mut self,
        engine: &mut Engine,
        response: Response,
    ) -> Result<Value, Fault> {
        let id = self.fetch.bodies.len();
        self.fetch.bodies.push(Some(response.body));
        let text = self.bind(engine, Native::Body(id, Body::Text));
        let json = self.bind(engine, Native::Body(id, Body::Json));
        let value = engine.object(
            [
                ("status".into(), Value::Number(response.status.into())),
                (
                    "ok".into(),
                    Value::Boolean((200..300).contains(&response.status)),
                ),
                ("mime".into(), Value::Text(response.mime)),
                ("text".into(), text),
                ("json".into(), json),
            ]
            .into(),
        )?;
        engine.freeze(&value)?;
        Ok(value)
    }
    pub(super) fn consume(
        &mut self,
        id: usize,
        format: Body,
        engine: &mut Engine,
    ) -> Result<Value, Fault> {
        let deferred = engine.deferred()?;
        let result = self.fetch.bodies[id]
            .take()
            .ok_or_else(|| "response body already consumed".to_string())
            .and_then(|body| decode(body, format, engine));
        let (callback, value) = match result {
            Ok(value) => (deferred.resolve, value),
            Err(message) => (
                deferred.reject,
                engine.exception(jsengine::Error::Type, Some(message))?,
            ),
        };
        engine
            .call(callback, vec![value], self)
            .map_err(|failure| self.diagnostics.diagnostic(engine, &failure))?;
        Ok(deferred.promise)
    }
}

fn decode(body: Vec<u8>, format: Body, engine: &mut Engine) -> Result<Value, String> {
    match format {
        Body::Text => String::from_utf8(body)
            .map(Value::Text)
            .map_err(|error| error.to_string()),
        Body::Json => serde_json::from_slice(&body)
            .map_err(|error| error.to_string())
            .and_then(|value| json(value, engine).map_err(|fault| fault.0)),
    }
}

fn json(value: serde_json::Value, engine: &mut Engine) -> Result<Value, Fault> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Boolean(value),
        serde_json::Value::Number(value) => Value::Number(
            value
                .as_f64()
                .ok_or_else(|| Fault("JSON number cannot be represented".into()))?,
        ),
        serde_json::Value::String(value) => Value::Text(value),
        serde_json::Value::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| json(value, engine).map(Some))
                .collect::<Result<Vec<_>, _>>()?;
            engine.array(values)?
        }
        serde_json::Value::Object(fields) => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, json(value, engine)?)))
                .collect::<Result<BTreeMap<_, _>, Fault>>()?;
            engine.object(fields)?
        }
    })
}
