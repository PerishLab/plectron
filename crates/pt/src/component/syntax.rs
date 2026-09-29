use crate::Fault;
use std::collections::BTreeSet;

pub(super) fn fields(source: &str) -> Result<Vec<(String, String)>, Fault> {
    let mut rest = source.trim();
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    while !rest.is_empty() {
        let (key, value) = rest
            .split_once('=')
            .ok_or_else(|| Fault("template attributes require values".into()))?;
        let event = key.strip_prefix('@');
        if !event.map_or_else(|| word(key), word) {
            return Err(Fault("attribute name must be a single word".into()));
        }
        if !seen.insert(key) {
            return Err(Fault(format!("duplicate attribute: {key}")));
        }
        let (value, next) = if event.is_some() {
            let value = value
                .strip_prefix('{')
                .ok_or_else(|| Fault("event handler requires braces".into()))?;
            let (value, next) = value
                .split_once('}')
                .ok_or_else(|| Fault("unterminated event handler".into()))?;
            if !word(value) {
                return Err(Fault("event handler must be a single identifier".into()));
            }
            (value, next)
        } else {
            let value = value
                .strip_prefix('"')
                .ok_or_else(|| Fault("template attributes require quoted values".into()))?;
            let (value, next) = value
                .split_once('"')
                .ok_or_else(|| Fault("unterminated attribute value".into()))?;
            if value.contains(['{', '}']) {
                return Err(Fault("template expressions are not supported".into()));
            }
            (value, next)
        };
        result.push((key.into(), value.into()));
        rest = next.trim();
    }
    Ok(result)
}

pub(super) fn word(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|ch: char| ch.is_ascii_alphabetic())
        && name
            .chars()
            .skip(1)
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

pub(super) fn literal(name: &str) -> bool {
    matches!(name, "id" | "class" | "dir" | "src" | "href" | "rel")
}

pub(super) fn intrinsic(name: &str) -> bool {
    matches!(
        name,
        "div" | "p" | "span" | "strong" | "a" | "button" | "img" | "h1" | "h2"
    )
}
