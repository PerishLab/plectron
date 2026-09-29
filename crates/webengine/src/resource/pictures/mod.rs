mod discovery;
use super::{Cancellation, Provider, Request, Response, Transport, Url};
use crate::{Fault, error, loading::Resource};
use domengine::Document;
use graphics::shared::Raster;
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct Pictures {
    base: Url,
    entries: BTreeMap<String, Raster>,
    transport: Option<Arc<dyn Transport>>,
    bytes: usize,
    version: Option<domengine::change::Version>,
    pending: std::collections::BTreeSet<domengine::Node>,
    pub(crate) report: super::Images,
}
impl Pictures {
    pub(crate) fn prune(&mut self, dead: &[domengine::Node]) {
        for node in dead {
            self.pending.remove(node);
        }
    }

    pub(crate) fn load(
        document: &mut Document,
        base: &Url,
        provider: &mut impl Provider,
        resources: &mut Vec<Resource>,
    ) -> Result<Self, Fault> {
        let mut pictures = Self {
            base: base.clone(),
            entries: BTreeMap::new(),
            transport: provider.transport(),
            bytes: 0,
            version: None,
            pending: Default::default(),
            report: super::Images::default(),
        };
        pictures.resolve(document, resources, &mut |url| {
            provider.request(&Request::new(url.clone(), super::Kind::Image))
        })?;
        Ok(pictures)
    }
    pub(crate) fn sync(
        &mut self,
        document: &mut Document,
        resources: &mut Vec<Resource>,
    ) -> Result<(), Fault> {
        let transport = self.transport.clone();
        self.resolve(document, resources, &mut |url| {
            let transport = transport.as_ref().ok_or_else(|| {
                Fault("image source is not loaded and provider has no retained transport".into())
            })?;
            transport.send(
                &Request::new(url.clone(), super::Kind::Image),
                &Cancellation::default(),
            )
        })
    }
    fn resolve(
        &mut self,
        document: &mut Document,
        resources: &mut Vec<Resource>,
        fetch: &mut impl FnMut(&Url) -> Result<Response, Fault>,
    ) -> Result<(), Fault> {
        self.discover(document)?;
        while let Some(node) = self.pending.first().copied() {
            if !document.connected(node).map_err(error)?
                || document.picture(node).map_err(error)?.is_some()
            {
                self.pending.remove(&node);
                continue;
            }
            let entry = document.entry(node).map_err(error)?;
            self.report.candidates += 1;
            let source = entry
                .attributes
                .get("src")
                .filter(|source| !source.is_empty())
                .cloned()
                .ok_or_else(|| Fault("img requires src".into()))?;
            let mut url = self.base.join(&source).map_err(error)?;
            url.set_fragment(None);
            let url = super::address(url.as_str())?;
            let key = url.to_string();
            if !self.entries.contains_key(&key) {
                self.acquire(&url, resources, fetch)?;
            }
            if document
                .image(node, &source, self.entries[&key].clone())
                .map_err(error)?
            {
                self.report.bindings += 1;
            }
            self.pending.remove(&node);
        }
        self.version = Some(document.version());
        Ok(())
    }
    fn acquire(
        &mut self,
        url: &Url,
        resources: &mut Vec<Resource>,
        fetch: &mut impl FnMut(&Url) -> Result<Response, Fault>,
    ) -> Result<(), Fault> {
        if resources.len() >= 32 {
            return Err(Fault("resource budget exhausted".into()));
        }
        self.report.fetches += 1;
        let response = Request::new(url.clone(), super::Kind::Image).accept(fetch(url)?)?;
        if response.status != 200
            || response.mime.split(';').next().map(str::trim) != Some("image/png")
        {
            return Err(Fault(format!(
                "image {url} requires a successful image/png response"
            )));
        }
        let raster = super::png::decode(&response.body)
            .map_err(|fault| Fault(format!("image {url}: {fault}")))?;
        if self.bytes + raster.retained() > 16_777_216 {
            return Err(Fault("page image residency budget exhausted".into()));
        }
        self.report.decoded += raster.retained() as u64;
        self.bytes += raster.retained();
        resources.push(Resource {
            url: url.to_string(),
            mime: response.mime,
            bytes: response.body.len(),
        });
        self.entries.insert(url.to_string(), raster);
        Ok(())
    }
}
