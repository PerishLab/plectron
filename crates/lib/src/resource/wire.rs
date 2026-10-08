use super::error;
use crate::contract::Fault;
use project::{Mapping, Module, Origin, Output, Span};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use webengine::resource::{Kind, Response, Script};

pub(super) const MIME: &str = "application/vnd.plectron.script+json";
pub(super) const LIMIT: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    javascript: String,
    modules: Vec<String>,
    origins: Vec<Segment>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    module: usize,
    generated: [usize; 2],
    source: [usize; 2],
    exact: bool,
}

pub(super) fn encode(output: Output) -> Result<Response, Fault> {
    if output.javascript.len() > Kind::Text.limit() {
        return Err(Fault("script byte budget exhausted".into()));
    }
    let mut modules = Vec::new();
    let mut indices = BTreeMap::new();
    let mut origins = Vec::new();
    for origin in output.origins {
        let index = *indices.entry(origin.module.clone()).or_insert_with(|| {
            let index = modules.len();
            modules.push(origin.module.path().into());
            index
        });
        origins.push(Segment {
            module: index,
            generated: [origin.generated.start, origin.generated.end],
            source: [origin.source.start, origin.source.end],
            exact: origin.mapping == Mapping::Exact,
        });
    }
    let body = serde_json::to_vec(&Envelope {
        version: 1,
        javascript: output.javascript,
        modules,
        origins,
    })
    .map_err(error)?;
    if body.len() > LIMIT {
        return Err(Fault("script transport byte budget exhausted".into()));
    }
    Ok(Response {
        status: 200,
        mime: MIME.into(),
        body,
    })
}

pub(super) fn decode(response: Response) -> Result<Script, Fault> {
    if response.mime != MIME {
        if response.body.len() > Kind::Text.limit() {
            return Err(Fault("response byte budget exhausted".into()));
        }
        return Ok(Script {
            response,
            source: None,
        });
    }
    if response.status != 200 || response.body.len() > LIMIT {
        return Err(Fault("invalid script transport response".into()));
    }
    let envelope: Envelope = serde_json::from_slice(&response.body).map_err(error)?;
    if envelope.version != 1 || envelope.javascript.len() > Kind::Text.limit() {
        return Err(Fault("unsupported script transport envelope".into()));
    }
    let modules: Vec<_> = envelope
        .modules
        .into_iter()
        .map(Module::new)
        .collect::<Result<_, _>>()
        .map_err(error)?;
    let mut origins = Vec::new();
    let mut end = 0;
    for Segment {
        module: index,
        generated,
        source,
        exact,
    } in envelope.origins
    {
        let generated = Span::new(generated[0], generated[1]).map_err(error)?;
        let source = Span::new(source[0], source[1]).map_err(error)?;
        let bounded = generated.end <= envelope.javascript.len()
            && envelope.javascript.is_char_boundary(generated.start)
            && envelope.javascript.is_char_boundary(generated.end);
        let consistent = !exact || generated.end - generated.start == source.end - source.start;
        if generated.start < end || !bounded || !consistent {
            return Err(Fault("invalid script transport mapping".into()));
        }
        let module = modules
            .get(index)
            .ok_or_else(|| Fault("invalid script transport module index".into()))?
            .clone();
        origins.push(Origin {
            mapping: if exact {
                Mapping::Exact
            } else {
                Mapping::Range
            },
            generated,
            module,
            source,
        });
        end = generated.end;
    }
    let body = envelope.javascript.as_bytes().to_vec();
    Ok(Script {
        response: Response {
            status: 200,
            mime: "text/javascript".into(),
            body,
        },
        source: Some(crate::runtime::source::mapped(envelope.javascript, origins)),
    })
}

#[cfg(test)]
mod tests;
