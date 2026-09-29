use crate::Fault;
use std::collections::BTreeMap;
use textengine::{Request, Weight};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub family: String,
    pub weight: Weight,
    pub source: String,
}
impl Font {
    pub(super) fn parse(mut properties: BTreeMap<String, String>) -> Result<Self, Fault> {
        let family = properties
            .remove("font-family")
            .ok_or_else(|| Fault("font-face requires font-family".into()))?;
        let family = unquote(family.trim())?.to_string();
        Request {
            families: vec![family.clone()].into(),
            ..Request::default()
        }
        .validate()
        .map_err(|fault| Fault(fault.0))?;
        let weight = match properties
            .remove("font-weight")
            .as_deref()
            .unwrap_or("normal")
        {
            "normal" | "400" => Weight::Normal,
            "bold" | "700" => Weight::Bold,
            _ => return Err(Fault("font-face supports normal or bold weight".into())),
        };
        let source = properties
            .remove("src")
            .ok_or_else(|| Fault("font-face requires src".into()))?;
        let source = source
            .strip_prefix("url(")
            .and_then(|value| value.strip_suffix(')'))
            .ok_or_else(|| Fault("font-face requires one url source".into()))?;
        let source = source.trim();
        if !source.starts_with(['\'', '"']) && source.chars().any(char::is_whitespace) {
            return Err(Fault("unquoted font URL contains whitespace".into()));
        }
        let source = unquote(source)?.to_string();
        if source.is_empty()
            || source.contains(['\\', '\n', '\r', '(', ')'])
            || !properties.is_empty()
        {
            return Err(Fault("unsupported font-face descriptor".into()));
        }
        Ok(Self {
            family,
            weight,
            source,
        })
    }
}
fn unquote(value: &str) -> Result<&str, Fault> {
    if let Some(quote) = value.chars().next().filter(|ch| matches!(ch, '\'' | '"')) {
        return value
            .strip_prefix(quote)
            .and_then(|value| value.strip_suffix(quote))
            .filter(|value| !value.contains(['\'', '"']))
            .ok_or_else(|| Fault("invalid font descriptor string".into()));
    }
    if value.contains(['\'', '"']) {
        return Err(Fault("invalid font descriptor string".into()));
    }
    Ok(value)
}
