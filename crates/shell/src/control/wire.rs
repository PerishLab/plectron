use super::json::{LIMIT, Value, encode as serialize, parse};
use super::receipt::{Receipt, Update, validate};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    Command {
        id: String,
        target: String,
        payload: Value,
    },
    Query {
        id: String,
    },
    Cancel {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub receipt: Receipt,
    pub duplicate: bool,
    pub found: bool,
}

pub fn decode(raw: &str) -> Result<Inbound, (String, String)> {
    if raw.len() > LIMIT {
        return Err((
            String::new(),
            format!("bridge message exceeds {LIMIT} byte budget"),
        ));
    }
    let parsed = parse(raw).map_err(|error| (String::new(), error))?;
    let Some(map) = parsed.object() else {
        return Err((String::new(), "bridge message must be a json object".into()));
    };
    let id = map
        .get("command_id")
        .and_then(Value::text)
        .map(str::to_owned)
        .unwrap_or_default();
    for key in map.keys() {
        if !matches!(key.as_str(), "kind" | "command_id" | "command" | "payload") {
            return Err((id, format!("unknown bridge field: {key}")));
        }
    }
    validate(&id, "command_id").map_err(|error| (String::new(), error))?;
    let kind = map.get("kind").and_then(Value::text).ok_or_else(|| {
        (
            id.clone(),
            "bridge message requires a string kind".to_string(),
        )
    })?;
    match kind {
        "query" => {
            if map.contains_key("command") || map.contains_key("payload") {
                return Err((
                    id,
                    "query message forbids command and payload fields".into(),
                ));
            }
            Ok(Inbound::Query { id })
        }
        "cancel" => {
            if map.contains_key("command") || map.contains_key("payload") {
                return Err((
                    id,
                    "cancel message forbids command and payload fields".into(),
                ));
            }
            Ok(Inbound::Cancel { id })
        }
        "command" => {
            let target = map.get("command").and_then(Value::text).ok_or_else(|| {
                (
                    id.clone(),
                    "command message requires a string command".to_string(),
                )
            })?;
            validate(target, "command").map_err(|error| (id.clone(), error))?;
            let payload = map.get("payload").cloned().unwrap_or(Value::Null);
            Ok(Inbound::Command {
                id,
                target: target.into(),
                payload,
            })
        }
        _ => Err((id, format!("unsupported bridge message kind: {kind}"))),
    }
}

pub fn encode(reply: &Reply) -> String {
    if let Ok(text) = serialize(&object(reply, true))
        && text.len() <= LIMIT
    {
        return text;
    }
    if let Ok(text) = serialize(&object(reply, false))
        && text.len() <= LIMIT
    {
        return text;
    }
    "{\"kind\":\"receipt\",\"status\":\"rejected\",\"error\":\"bridge encoding failed\"}".into()
}

fn object(reply: &Reply, full: bool) -> Value {
    let value = if full {
        reply.receipt.value.clone().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let error = if full {
        reply
            .receipt
            .error
            .clone()
            .map(Value::Text)
            .unwrap_or(Value::Null)
    } else {
        Value::Text(format!("receipt payload exceeds {LIMIT} byte budget"))
    };
    let progress = if full {
        reply
            .receipt
            .progress
            .as_ref()
            .map(update)
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    Value::Object(BTreeMap::from([
        ("kind".into(), Value::Text("receipt".into())),
        (
            "command_id".into(),
            Value::Text(reply.receipt.command.clone()),
        ),
        ("command".into(), Value::Text(reply.receipt.target.clone())),
        (
            "status".into(),
            Value::Text(reply.receipt.status.label().into()),
        ),
        ("duplicate".into(), Value::Bool(reply.duplicate)),
        ("found".into(), Value::Bool(reply.found)),
        ("value".into(), value),
        ("error".into(), error),
        ("progress".into(), progress),
    ]))
}

fn update(progress: &Update) -> Value {
    let total = progress
        .total
        .map(|value| Value::Number(value as f64))
        .unwrap_or(Value::Null);
    let message = progress
        .message
        .clone()
        .map(Value::Text)
        .unwrap_or(Value::Null);
    Value::Object(BTreeMap::from([
        ("current".into(), Value::Number(progress.current as f64)),
        ("total".into(), total),
        ("message".into(), message),
    ]))
}
