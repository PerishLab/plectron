use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use webengine::{
    Fault,
    resource::{Cancellation, Provider, Request, Response, Transport},
};

#[cfg(test)]
mod tests;

struct Loaded {
    response: Response,
    script: Option<project::Output>,
    normalized: Option<Vec<u8>>,
}

#[derive(Clone, Default)]
pub(crate) struct Source {
    assets: Arc<Mutex<BTreeMap<String, Usage>>>,
    normalized: bool,
}
#[derive(Clone)]
struct Usage {
    asset: Asset,
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
    fn script(
        &mut self,
        url: &webengine::resource::Url,
    ) -> Result<webengine::resource::Script, Fault> {
        let request = Request::new(url.clone(), webengine::resource::Kind::Text);
        let (response, script) = self.resource(&request, &Cancellation::default())?;
        Ok(webengine::resource::Script {
            response,
            source: script.map(plectron::runtime::source::script),
        })
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
                let loaded = self.load(&address, usage.limit)?;
                if loaded.response.body.len() != usage.asset.bytes
                    || fingerprint(&loaded.response.body) != usage.asset.fingerprint
                {
                    return Err(Fault(format!("scenario asset changed during run: {url}")));
                }
                Ok((url, usage.asset))
            })
            .collect()
    }
    fn load(&self, url: &webengine::resource::Url, limit: usize) -> Result<Loaded, Fault> {
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
        let script = if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts" | "pt")
        ) {
            let mut workspace = project::Workspace::open(
                &path,
                project::Adapters::new(typescript::Typescript, pt::Templates),
            )
            .map_err(error)?;
            Some(workspace.link(project::Target::Script).map_err(error)?)
        } else {
            None
        };
        let mut body = match &script {
            Some(script) => script.javascript.as_bytes().to_vec(),
            None => std::fs::read(&path).map_err(error)?,
        };
        let mut normalized = None;
        if self.normalized && url.path().ends_with("/style.css") {
            let bytes = std::fs::read(root.join("benchmarks/normalize.css")).map_err(error)?;
            body.push(b'\n');
            body.extend_from_slice(&bytes);
            normalized = Some(bytes);
        }
        Ok(Loaded {
            response: Response {
                status: 200,
                mime: mime.into(),
                body,
            },
            script,
            normalized,
        })
    }
    fn record(&self, url: String, body: &[u8], limit: usize) -> Result<(), Fault> {
        let mut assets = self.assets.lock().expect("scenario asset lock");
        let signature = fingerprint(body);
        let usage = assets.entry(url.clone()).or_insert(Usage {
            asset: Asset {
                bytes: body.len(),
                fingerprint: signature,
                requests: 0,
            },
            limit,
        });
        let asset = &mut usage.asset;
        if asset.bytes != body.len() || asset.fingerprint != signature {
            return Err(Fault(format!("scenario asset changed during run: {url}")));
        }
        asset.requests += 1;
        Ok(())
    }
    fn resource(
        &self,
        request: &Request,
        cancellation: &Cancellation,
    ) -> Result<(Response, Option<project::Output>), Fault> {
        cancellation.check()?;
        let limit = request.kind.limit();
        let loaded = self.load(&request.url, limit)?;
        if loaded.response.body.len() > request.kind.limit() {
            return Err(Fault("scenario resource exceeds request budget".into()));
        }
        cancellation.check()?;
        self.record(request.url.to_string(), &loaded.response.body, limit)?;
        if let Some(bytes) = loaded.normalized {
            self.record(
                "app://scenarios/benchmarks/normalize.css".into(),
                &bytes,
                webengine::resource::Kind::Text.limit(),
            )?;
        }
        Ok((loaded.response, loaded.script))
    }
}
impl Transport for Source {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        self.resource(request, cancellation)
            .map(|(response, _)| response)
    }
}
fn mime(path: &std::path::Path) -> Result<&'static str, Fault> {
    match path.extension().and_then(|v| v.to_str()) {
        Some("html") => Ok("text/html"),
        Some("css") => Ok("text/css"),
        Some("js" | "ts" | "pt") => Ok("text/javascript"),
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
