use super::{Services, authority, http, wire};
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
    fn script(
        &mut self,
        url: &webengine::resource::Url,
    ) -> Result<webengine::resource::Script, Fault> {
        wire::decode(self.load(
            &webengine::resource::Request::new(url.clone(), webengine::resource::Kind::Text),
            &webengine::resource::Cancellation::default(),
            Some((wire::MIME, wire::LIMIT)),
        )?)
    }
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
        let response = self.load(request, cancellation, None)?;
        request.accept(response)
    }
}

impl Loader {
    fn load(
        &self,
        request: &webengine::resource::Request,
        cancellation: &webengine::resource::Cancellation,
        accept: Option<(&str, usize)>,
    ) -> Result<Response, Fault> {
        let url = &request.url;
        cancellation.check()?;
        if request.url.scheme() == "http" {
            return http::fetch(request, cancellation, accept);
        }
        let key = authority(&request.url)?;
        let response = if let Some(address) = self.routes.get(&key) {
            http::load(
                *address,
                (
                    &http::path(url),
                    accept.map_or(request.kind.limit(), |(_, limit)| limit),
                ),
                cancellation,
                accept.map(|(mime, _)| mime),
            )?
        } else {
            return Err(Fault(format!("unregistered scheme authority: {key}")));
        };
        cancellation.check()?;
        Ok(response)
    }
}
