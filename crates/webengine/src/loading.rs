use crate::resource::{Provider, Url};
use crate::{Fault, error};
use domengine::{Document, Kind, Sheet, html};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub url: String,
    pub mime: String,
    pub bytes: usize,
}

pub struct Loaded {
    pub document: Document,
    pub sheet: Sheet,
    pub scripts: Vec<(String, Option<jsruntime::Script>)>,
    pub fonts: Vec<(Url, domengine::style::Font)>,
    pub resources: Vec<Resource>,
}

impl Loaded {
    pub fn fetch(url: &Url, provider: &mut impl Provider) -> Result<Self, Fault> {
        let mut resources = Vec::new();
        let (source, _) = fetch(provider, url, "text/html", &mut resources)?;
        let document = html::parse(&source).map_err(error)?;
        let mut sheet = Sheet::default();
        let mut fonts = Vec::new();
        let mut scripts = Vec::new();
        let assets = assets(&document)?;
        for (path, kind) in assets {
            let target = url.join(&path).map_err(error)?;
            let (source, script) = fetch(provider, &target, kind, &mut resources)?;
            if kind == "text/css" {
                let parsed = Sheet::parse(&source).map_err(error)?;
                fonts.extend(
                    parsed
                        .fonts()
                        .iter()
                        .cloned()
                        .map(|font| (target.clone(), font)),
                );
                sheet.extend(parsed).map_err(error)?;
            } else {
                scripts.push((source, script));
            }
        }
        Ok(Self {
            document,
            sheet,
            scripts,
            fonts,
            resources,
        })
    }
}

fn assets(document: &Document) -> Result<Vec<(String, &'static str)>, Fault> {
    let mut styles = Vec::new();
    let mut scripts = Vec::new();
    for node in document.walk() {
        let entry = document.entry(node).map_err(error)?;
        match &entry.kind {
            Kind::Element(tag) if tag == "link" => {
                stylesheet(entry)?;
                styles.push((required(&entry.attributes, "href")?, "text/css"));
            }
            Kind::Element(tag) if tag == "script" => {
                external(entry)?;
                scripts.push((required(&entry.attributes, "src")?, "text/javascript"));
            }
            Kind::Element(tag) if tag == "style" => {
                return Err(Fault("use an external stylesheet".into()));
            }
            _ => (),
        }
    }
    styles.extend(scripts);
    Ok(styles)
}

fn required(
    attributes: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<String, Fault> {
    attributes
        .get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| Fault(format!("missing {name}")))
}

fn fetch(
    provider: &mut impl Provider,
    url: &Url,
    mime: &str,
    resources: &mut Vec<Resource>,
) -> Result<(String, Option<jsruntime::Script>), Fault> {
    if resources.len() >= 32 {
        return Err(Fault("resource budget exhausted".into()));
    }
    let (response, script) = if mime == "text/javascript" {
        let script = provider.script(url)?;
        (script.response, script.source)
    } else {
        (provider.get(url)?, None)
    };
    if response.status != 200 {
        return Err(Fault(format!(
            "resource {url} returned {}",
            response.status
        )));
    }
    if response.mime.split(';').next().map(str::trim) != Some(mime) {
        return Err(Fault(format!("resource {url} requires {mime}")));
    }
    if response.body.len() > 65_536 {
        return Err(Fault("resource byte budget exhausted".into()));
    }
    resources.push(Resource {
        url: url.to_string(),
        mime: response.mime,
        bytes: response.body.len(),
    });
    let source = String::from_utf8(response.body).map_err(error)?;
    if script
        .as_ref()
        .is_some_and(|script| script.source().text() != source)
    {
        return Err(Fault(format!(
            "resource {url} source metadata does not match body"
        )));
    }
    Ok((source, script))
}

fn stylesheet(entry: &domengine::Entry) -> Result<(), Fault> {
    if entry.attributes.get("rel").map(String::as_str) != Some("stylesheet") {
        return Err(Fault("link requires rel=stylesheet".into()));
    }
    Ok(())
}

fn external(entry: &domengine::Entry) -> Result<(), Fault> {
    if !entry.children.is_empty() {
        return Err(Fault("only external scripts are supported".into()));
    }
    Ok(())
}
