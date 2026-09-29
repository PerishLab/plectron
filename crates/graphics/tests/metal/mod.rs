#![cfg(target_os = "macos")]

mod application;
mod typography;

#[derive(Clone)]
struct Source(fixtures::Application);

impl Source {
    fn new(name: &str) -> Self {
        Self(fixtures::Application::new(name))
    }
}

impl webengine::resource::Provider for Source {
    fn transport(&self) -> Option<std::sync::Arc<dyn webengine::resource::Transport>> {
        Some(std::sync::Arc::new(self.clone()))
    }

    fn request(
        &mut self,
        request: &webengine::resource::Request,
    ) -> Result<webengine::resource::Response, webengine::Fault> {
        webengine::resource::Transport::send(
            self,
            request,
            &webengine::resource::Cancellation::default(),
        )
    }
}

impl webengine::resource::Transport for Source {
    fn send(
        &self,
        request: &webengine::resource::Request,
        cancellation: &webengine::resource::Cancellation,
    ) -> Result<webengine::resource::Response, webengine::Fault> {
        cancellation.check()?;
        let resource = self.0.load(request.url.path()).map_err(webengine::Fault)?;
        request.accept(webengine::resource::Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
}
