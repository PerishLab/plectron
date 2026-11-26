use crate::bundle::{Bundle, authority};
use crate::fonts::System;
use std::sync::Arc;
use webengine::Fault;
use webengine::resource::{Cancellation, Provider, Request, Response, Transport};

#[derive(Clone)]
pub struct Memory {
    bundle: Arc<Bundle>,
    fonts: Option<Arc<dyn textengine::Resolver>>,
}

impl Memory {
    pub fn new(bundle: Arc<Bundle>) -> Self {
        Self {
            bundle,
            fonts: None,
        }
    }

    pub fn system(mut self) -> Self {
        self.fonts = Some(Arc::new(System::new()));
        self
    }

    pub fn fonts(mut self, resolver: Option<Arc<dyn textengine::Resolver>>) -> Self {
        self.fonts = resolver;
        self
    }

    pub fn entry(&self) -> &str {
        &self.bundle.entry
    }
}

impl Provider for Memory {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        Transport::send(self, request, &Cancellation::default())
    }

    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(Arc::new(self.clone()))
    }

    fn fonts(&self) -> Option<Arc<dyn textengine::Resolver>> {
        self.fonts.clone()
    }
}

impl Transport for Memory {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        cancellation.check()?;
        let origin = authority(&request.url)?;
        if origin != self.bundle.origin {
            return Err(Fault(format!("unregistered scheme authority: {origin}")));
        }
        let response = match self.bundle.get(request.url.path()) {
            Some(asset) => Response {
                status: 200,
                mime: asset.mime.clone(),
                body: asset.body.clone(),
            },
            None => Response {
                status: 404,
                mime: "text/plain".into(),
                body: b"resource status 404".to_vec(),
            },
        };
        cancellation.check()?;
        request.accept(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webengine::Page;

    #[test]
    fn loading() {
        let mut bundle = Bundle::new("app://demo/", "app://demo/index.html").unwrap();
        bundle
            .insert(
                "/index.html",
                "text/html",
                b"<!doctype html><html><head><link rel='stylesheet' href='app.css'><script src='app.js'></script></head><body><div id='root'></div></body></html>".to_vec(),
            )
            .unwrap();
        bundle
            .insert(
                "/app.css",
                "text/css",
                b"body { margin: 0; background-color: #ffffff; color: #111111; }".to_vec(),
            )
            .unwrap();
        bundle
            .insert(
                "/app.js",
                "text/javascript",
                b"Symbol[Symbol.plectron].document.getElementById('root').setText('embedded');"
                    .to_vec(),
            )
            .unwrap();
        let mut provider = Memory::new(Arc::new(bundle));
        let mut page = Page::new(
            provider.entry(),
            domengine::Extent {
                width: 320,
                height: 200,
            },
        )
        .unwrap();
        page.load(&mut provider).unwrap();
        assert_eq!(page.state(), webengine::State::Ready);
        assert!(page.frame().is_some());
    }
}
