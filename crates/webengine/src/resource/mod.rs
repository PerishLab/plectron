use crate::{Fault, error};
pub(crate) mod fonts;
mod pictures;
mod report;
pub use report::Images;
mod png;
mod request;
pub(crate) use pictures::Pictures;
pub use request::{Cancellation, Kind, Request, Transport};
pub use url::Url;

pub struct Response {
    pub status: u16,
    pub mime: String,
    pub body: Vec<u8>,
}

pub struct Script {
    pub response: Response,
    pub source: Option<jsruntime::Script>,
}

pub trait Provider {
    fn request(&mut self, request: &Request) -> Result<Response, Fault>;
    fn get(&mut self, url: &Url) -> Result<Response, Fault> {
        let request = Request::new(url.clone(), Kind::Text);
        request.accept(self.request(&request)?)
    }
    fn script(&mut self, url: &Url) -> Result<Script, Fault> {
        Ok(Script {
            response: self.get(url)?,
            source: None,
        })
    }
    fn transport(&self) -> Option<std::sync::Arc<dyn Transport>> {
        None
    }
}

pub fn address(source: &str) -> Result<Url, Fault> {
    let url = Url::parse(source).map_err(error)?;
    if url.cannot_be_a_base() || url.host_str().is_none() {
        return Err(Fault(
            "content URL requires a hierarchical authority".into(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Fault("URL credentials are unsupported".into()));
    }
    Ok(url)
}
