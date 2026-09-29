use super::{Response, Url};
use crate::Fault;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone)]
pub struct Request {
    pub url: Url,
    pub kind: Kind,
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), Fault> {
        if self.cancelled() {
            Err(Fault("request cancelled".into()))
        } else {
            Ok(())
        }
    }
}
pub trait Transport: Send + Sync {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault>;
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Text,
    Font,
    Image,
}
impl Kind {
    pub fn limit(self) -> usize {
        match self {
            Self::Text => 65_536,
            Self::Font => 16_777_216,
            Self::Image => 4_194_304,
        }
    }
}
impl Request {
    pub fn new(url: Url, kind: Kind) -> Self {
        Self { url, kind }
    }
    pub fn accept(&self, response: Response) -> Result<Response, Fault> {
        if response.body.len() > self.kind.limit() {
            return Err(Fault("response byte budget exhausted".into()));
        }
        Ok(response)
    }
}
