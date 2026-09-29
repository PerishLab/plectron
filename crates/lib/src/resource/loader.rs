use super::{Services, authority, http};
use crate::contract::Fault;
use std::collections::BTreeMap;
use std::net::SocketAddr as Address;
use webengine::resource::{Provider, Response};

#[derive(Clone)]
pub struct Loader {
    routes: BTreeMap<String, Address>,
}
impl Services {
    pub fn loader(&self) -> Loader {
        Loader {
            routes: self
                .routes
                .iter()
                .filter_map(|(key, id)| {
                    self.servers
                        .get(id)
                        .map(|server| (key.clone(), server.address()))
                })
                .collect(),
        }
    }
}
impl Provider for Loader {
    fn transport(&self) -> Option<std::sync::Arc<dyn webengine::resource::Transport>> {
        Some(std::sync::Arc::new(self.clone()))
    }
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        webengine::resource::Transport::send(
            self,
            request,
            &webengine::resource::Cancellation::default(),
        )
    }
}

impl webengine::resource::Transport for Loader {
    fn send(
        &self,
        request: &webengine::resource::Request,
        cancellation: &webengine::resource::Cancellation,
    ) -> Result<Response, Fault> {
        let url = &request.url;
        cancellation.check()?;
        if request.url.scheme() == "http" {
            return http::fetch(request, cancellation);
        }
        let key = authority(&request.url)?;
        let response = if let Some(address) = self.routes.get(&key) {
            http::load(
                *address,
                (&http::path(url), request.kind.limit()),
                cancellation,
            )?
        } else {
            return Err(Fault(format!("unregistered scheme authority: {key}")));
        };
        cancellation.check()?;
        request.accept(response)
    }
}
