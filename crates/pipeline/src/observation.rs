use std::time::{Duration, Instant};

pub trait Observer {
    fn reset(&mut self) {}
    fn begin(&mut self, _: impl FnOnce() -> crate::Topology) {}
    fn node<T, E>(
        &mut self,
        identity: (usize, &'static str),
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E>;
}

pub struct Silent;
impl Observer for Silent {
    fn node<T, E>(
        &mut self,
        _: (usize, &'static str),
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        execute()
    }
}

#[derive(Debug)]
pub struct Span {
    pub index: usize,
    pub name: &'static str,
    pub elapsed: Duration,
    pub success: bool,
}

#[derive(Default, Debug)]
pub struct Trace {
    pub topology: Option<crate::Topology>,
    pub spans: Vec<Span>,
}
impl Observer for Trace {
    fn reset(&mut self) {
        self.topology = None;
        self.spans.clear();
    }
    fn begin(&mut self, topology: impl FnOnce() -> crate::Topology) {
        self.topology = Some(topology());
    }
    fn node<T, E>(
        &mut self,
        identity: (usize, &'static str),
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let start = Instant::now();
        let result = execute();
        self.spans.push(Span {
            index: identity.0,
            name: identity.1,
            elapsed: start.elapsed(),
            success: result.is_ok(),
        });
        result
    }
}
