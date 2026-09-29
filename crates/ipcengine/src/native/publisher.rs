use super::{channel, mapping::Mapping, rights};
use crate::{
    Fault,
    observe::{Lease, Monitor},
};
use serde::Serialize;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Phase {
    Writable,
    Published,
    Retired,
}
pub struct Publisher {
    mapping: Option<Mapping>,
    socket: UnixStream,
    generation: u64,
    phase: Phase,
    monitor: Monitor,
    lease: Option<Lease>,
}
impl Publisher {
    pub fn new(socket: UnixStream, directory: &Path, length: usize) -> Result<Self, Fault> {
        channel::prepare(&socket)?;
        let mapping = Mapping::allocate(directory, length)?;
        Ok(Self {
            mapping: Some(mapping),
            socket,
            generation: 0,
            phase: Phase::Writable,
            monitor: Monitor::default(),
            lease: None,
        })
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn monitor(&self) -> Monitor {
        self.monitor.clone()
    }
    fn writable(&mut self) -> Result<&mut Mapping, Fault> {
        if self.phase != Phase::Writable {
            return Err(Fault("shared buffer is not writable".into()));
        }
        self.mapping
            .as_mut()
            .ok_or_else(|| Fault("shared buffer retired".into()))
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        self.writable()?.write(bytes)
    }
    pub fn fill(&mut self, value: u8) -> Result<(), Fault> {
        self.writable()?.fill(value)
    }
    pub fn publish(&mut self) -> Result<u64, Fault> {
        self.writable()?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Fault("shared generations exhausted".into()))?;
        self.phase = Phase::Published;
        self.lease = Some(self.monitor.lease());
        if let Err(error) = self.offer() {
            self.retire(&error.to_string());
            return Err(error);
        }
        Ok(self.generation)
    }
    fn offer(&mut self) -> Result<(), Fault> {
        let mapping = self.mapping.as_ref().expect("published mapping");
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        rights::Rights(&self.socket).send(mapping.descriptor())?;
        let mut header = [0; 24];
        header[..8].copy_from_slice(b"PLBUF001");
        header[8..16].copy_from_slice(&self.generation.to_be_bytes());
        header[16..].copy_from_slice(&(mapping.length() as u64).to_be_bytes());
        channel::write(&self.socket, &header)?;
        self.lease
            .as_mut()
            .expect("publication lease")
            .bytes(mapping.length());
        Ok(())
    }
    pub fn reclaim(&mut self) -> Result<(), Fault> {
        if self.phase != Phase::Published {
            return Err(Fault("no published shared buffer".into()));
        }
        let started = Instant::now();
        let result = self.acknowledge();
        self.lease
            .as_ref()
            .expect("publication lease")
            .wait(started);
        if let Err(error) = result {
            self.retire(&error.to_string());
            return Err(error);
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        self.lease.as_mut().expect("publication lease").finish();
        self.lease = None;
        self.phase = Phase::Writable;
        Ok(())
    }
    fn acknowledge(&self) -> Result<(), Fault> {
        let mut header = [0; 16];
        channel::read(&self.socket, &mut header)?;
        if &header[..8] != b"PLRET001"
            || u64::from_be_bytes(header[8..].try_into().expect("generation")) != self.generation
        {
            return Err(Fault("stale or invalid shared-buffer return".into()));
        }
        Ok(())
    }
    fn retire(&mut self, reason: &str) {
        if let Some(lease) = &mut self.lease {
            lease.fail(reason);
        }
        self.lease = None;
        self.mapping = None;
        self.phase = Phase::Retired;
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
impl Drop for Publisher {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
