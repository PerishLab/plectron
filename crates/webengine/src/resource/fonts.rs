use super::{Kind, Provider, Request, Url};
use crate::{Fault, error, loading::Resource};
use std::collections::BTreeMap;
use textengine::{Catalog, Face, font::Lease};

pub(crate) fn load(
    declarations: &[(Url, domengine::style::Font)],
    provider: &mut impl Provider,
    catalog: &Catalog,
    resources: &mut Vec<Resource>,
) -> Result<crate::Loading, Fault> {
    let mut report = crate::Loading::default();
    let mut sources = BTreeMap::new();
    let mut bytes = 0;
    for (base, font) in declarations {
        let mut url = base.join(&font.source).map_err(error)?;
        url.set_fragment(None);
        let url = super::address(url.as_str())?;
        if !sources.contains_key(url.as_str()) {
            if resources.len() >= 32 {
                return Err(Fault("resource budget exhausted".into()));
            }
            let request = Request::new(url.clone(), Kind::Font);
            let start = std::time::Instant::now();
            let response = request.accept(provider.request(&request)?)?;
            report.requests = report.requests.saturating_add(crate::page::elapsed(start));
            if response.status != 200
                || response.mime.split(';').next().map(str::trim) != Some("font/ttf")
            {
                return Err(Fault(format!(
                    "font {url} requires a successful font/ttf response"
                )));
            }
            if bytes + response.body.len() > 67_108_864 {
                return Err(Fault("page font residency budget exhausted".into()));
            }
            let length = response.body.len();
            let start = std::time::Instant::now();
            let lease = Lease::new(response.body.into())
                .map_err(|fault| Fault(format!("font {url}: {fault}")))?;
            report.parsing = report.parsing.saturating_add(crate::page::elapsed(start));
            bytes += length;
            report.fonts += 1;
            report.bytes = report.bytes.saturating_add(length);
            resources.push(Resource {
                url: url.to_string(),
                mime: response.mime,
                bytes: length,
            });
            sources.insert(url.to_string(), lease);
        }
        let start = std::time::Instant::now();
        catalog
            .register(
                &font.family,
                font.weight,
                Face::Owned(sources[url.as_str()].clone()),
            )
            .map_err(error)?;
        report.registration = report
            .registration
            .saturating_add(crate::page::elapsed(start));
    }
    Ok(report)
}
