use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use webengine::{
    Fault,
    resource::{Cancellation, Provider, Request, Response, Transport},
};

#[derive(Clone, Default)]
pub(crate) struct Source {
    assets: Arc<Mutex<BTreeMap<String, Usage>>>,
    normalized: bool,
}
#[derive(Clone)]
struct Usage {
    bytes: usize,
    requests: usize,
    limit: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct Asset {
    pub bytes: usize,
    pub fingerprint: u64,
    pub requests: usize,
}
impl Provider for Source {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        self.send(request, &Cancellation::default())
    }
    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(Arc::new(self.clone()))
    }
}
impl Source {
    pub fn configured(config: &config::Store) -> Result<Self, Fault> {
        let normalized = config
            .get("scenario.normalized")
            .map(serde_json::from_value)
            .transpose()
            .map_err(error)?
            .unwrap_or(false);
        Ok(Self {
            normalized,
            ..Self::default()
        })
    }
    pub fn assets(&self) -> Result<BTreeMap<String, Asset>, Fault> {
        let assets = self.assets.lock().expect("scenario asset lock").clone();
        assets
            .into_iter()
            .map(|(url, usage)| {
                let address = webengine::resource::address(&url)?;
                let (_, body, _) = self.load(&address, usage.limit)?;
                if body.len() != usage.bytes {
                    return Err(Fault(format!("scenario asset changed during run: {url}")));
                }
                Ok((
                    url,
                    Asset {
                        bytes: usage.bytes,
                        fingerprint: fingerprint(&body),
                        requests: usage.requests,
                    },
                ))
            })
            .collect()
    }
    fn load(
        &self,
        url: &webengine::resource::Url,
        limit: usize,
    ) -> Result<(&'static str, Vec<u8>, Option<usize>), Fault> {
        if url.scheme() != "app" || url.host_str() != Some("scenarios") {
            return Err(Fault(
                "scenario provider only serves app://scenarios".into(),
            ));
        }
        let root = crate::root().join("apps").canonicalize().map_err(error)?;
        let path = root
            .join(url.path().trim_start_matches('/'))
            .canonicalize()
            .map_err(error)?;
        if !path.starts_with(&root) || path.metadata().map_err(error)?.len() > limit as u64 {
            return Err(Fault(
                "scenario resource outside root or request budget".into(),
            ));
        }
        let mime = mime(&path)?;
        let mut body = if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "pt")
        ) {
            let mut workspace = project::Workspace::open(&path, pt::Templates).map_err(error)?;
            workspace
                .link(project::Target::Script)
                .map_err(error)?
                .javascript
                .into_bytes()
        } else {
            std::fs::read(&path).map_err(error)?
        };
        let mut normalized = None;
        if self.normalized && url.path().ends_with("/style.css") {
            let bytes = std::fs::read(root.join("benchmarks/normalize.css")).map_err(error)?;
            normalized = Some(bytes.len());
            body.push(b'\n');
            body.extend(bytes);
        }
        Ok((mime, body, normalized))
    }
    fn record(&self, url: String, bytes: usize, limit: usize) {
        let mut assets = self.assets.lock().expect("scenario asset lock");
        let usage = assets.entry(url).or_insert(Usage {
            bytes,
            requests: 0,
            limit,
        });
        usage.requests += 1;
    }
}
impl Transport for Source {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        cancellation.check()?;
        let limit = request.kind.limit();
        let (mime, body, normalized) = self.load(&request.url, limit)?;
        if body.len() > request.kind.limit() {
            return Err(Fault("scenario resource exceeds request budget".into()));
        }
        cancellation.check()?;
        self.record(request.url.to_string(), body.len(), limit);
        if let Some(bytes) = normalized {
            self.record(
                "app://scenarios/benchmarks/normalize.css".into(),
                bytes,
                webengine::resource::Kind::Text.limit(),
            );
        }
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body,
        })
    }
}
fn mime(path: &std::path::Path) -> Result<&'static str, Fault> {
    match path.extension().and_then(|v| v.to_str()) {
        Some("html") => Ok("text/html"),
        Some("css") => Ok("text/css"),
        Some("js" | "pt") => Ok("text/javascript"),
        Some("json") => Ok("application/json"),
        Some("ttf") => Ok("font/ttf"),
        Some("png") => Ok("image/png"),
        _ => Err(Fault("unsupported scenario resource type".into())),
    }
}
pub(crate) fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037, |hash, byte| {
        hash.wrapping_mul(1099511628211) ^ u64::from(*byte)
    })
}
fn error(value: impl std::fmt::Display) -> Fault {
    Fault(value.to_string())
}
