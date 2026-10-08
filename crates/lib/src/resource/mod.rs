mod http;
mod loader;
pub use loader::Loader;
mod server;
mod wire;
use crate::contract::Fault;
use serde::{Deserialize, Serialize};
pub use server::Server;
use std::collections::BTreeMap;
use std::path::Path;
use webengine::resource::{Provider, Response, Url, address};

#[derive(Default)]
pub struct Services {
    servers: BTreeMap<u64, Server>,
    routes: BTreeMap<String, u64>,
    sequence: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    pub id: u64,
    pub address: String,
    pub schemes: Vec<String>,
}

impl Services {
    pub fn serve(&mut self, root: &Path) -> Result<u64, Fault> {
        if self.servers.len() >= 8 {
            return Err(Fault("server budget exhausted".into()));
        }
        let server = Server::start(root)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Fault("service IDs exhausted".into()))?;
        self.servers.insert(self.sequence, server);
        Ok(self.sequence)
    }

    pub fn scheme(&mut self, id: u64, source: &str) -> Result<(), Fault> {
        if !self.servers.contains_key(&id) {
            return Err(Fault("unknown or closed service".into()));
        }
        let url = address(source)?;
        if matches!(url.scheme(), "http" | "https" | "file") {
            return Err(Fault("register a custom scheme".into()));
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(Fault("scheme mapping requires an authority root".into()));
        }
        let key = authority(&url)?;
        if self.routes.contains_key(&key) {
            return Err(Fault("scheme mapping already registered".into()));
        }
        if self.routes.len() >= 32 {
            return Err(Fault("scheme budget exhausted".into()));
        }
        self.routes.insert(key, id);
        Ok(())
    }

    pub fn close(&mut self, id: u64) -> Result<(), Fault> {
        self.servers
            .remove(&id)
            .ok_or_else(|| Fault("unknown or closed service".into()))?;
        self.routes.retain(|_, service| *service != id);
        Ok(())
    }

    pub fn inspect(&self) -> Vec<Service> {
        self.servers
            .iter()
            .map(|(id, server)| Service {
                id: *id,
                address: format!("http://{}", server.address()),
                schemes: self
                    .routes
                    .iter()
                    .filter(|(_, service)| *service == id)
                    .map(|(route, _)| route.clone())
                    .collect(),
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.routes.clear();
        self.servers.clear();
    }
}

impl Provider for Services {
    fn script(&mut self, url: &Url) -> Result<webengine::resource::Script, Fault> {
        self.loader().script(url)
    }
    fn transport(&self) -> Option<std::sync::Arc<dyn webengine::resource::Transport>> {
        self.loader().transport()
    }
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        self.loader().request(request)
    }
}

fn authority(url: &Url) -> Result<String, Fault> {
    if url.port().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err(Fault(
            "custom authority excludes ports and credentials".into(),
        ));
    }
    Ok(format!(
        "{}://{}/",
        url.scheme(),
        url.host_str()
            .ok_or_else(|| Fault("URL has no authority".into()))?
    ))
}

fn error(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}
