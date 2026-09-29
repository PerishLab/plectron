use super::{channel, mapping::Mapping, rights};
use crate::{
    Fault,
    observe::{self, Monitor},
};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::Instant;

pub struct Receiver {
    socket: UnixStream,
    generation: u64,
    closed: bool,
    monitor: Monitor,
}
pub struct Lease<'a> {
    receiver: &'a mut Receiver,
    mapping: Option<Mapping>,
    observation: observe::Lease,
    returned: bool,
}
impl Receiver {
    pub fn new(socket: UnixStream) -> Result<Self, Fault> {
        channel::prepare(&socket)?;
        Ok(Self {
            socket,
            generation: 0,
            closed: false,
            monitor: Monitor::default(),
        })
    }
    pub fn monitor(&self) -> Monitor {
        self.monitor.clone()
    }
    pub fn acquire(&mut self) -> Result<Lease<'_>, Fault> {
        if self.closed {
            return Err(Fault("shared receiver is closed".into()));
        }
        let mut observation = self.monitor.lease();
        let started = Instant::now();
        let result = self.receive();
        observation.wait(started);
        match result {
            Ok(mapping) => {
                observation.bytes(mapping.length());
                Ok(Lease {
                    receiver: self,
                    mapping: Some(mapping),
                    observation,
                    returned: false,
                })
            }
            Err(error) => {
                observation.fail(&error.to_string());
                self.close();
                Err(error)
            }
        }
    }
    fn receive(&mut self) -> Result<Mapping, Fault> {
        let file = rights::Rights(&self.socket).receive()?;
        let mut header = [0; 24];
        channel::read(&self.socket, &mut header)?;
        let generation = u64::from_be_bytes(header[8..16].try_into().expect("generation"));
        let length = u64::from_be_bytes(header[16..].try_into().expect("length"));
        if &header[..8] != b"PLBUF001"
            || Some(generation) != self.generation.checked_add(1)
            || length > 16_777_216
        {
            return Err(Fault("invalid shared-buffer publication".into()));
        }
        let mapping = Mapping::receive(file, length as usize)?;
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        self.generation = generation;
        Ok(mapping)
    }
    fn close(&mut self) {
        self.closed = true;
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
impl Lease<'_> {
    pub fn generation(&self) -> u64 {
        self.receiver.generation
    }
    pub fn length(&self) -> usize {
        self.mapping.as_ref().expect("live mapping").length()
    }
    pub fn copy(&self, bytes: &mut [u8]) -> Result<(), Fault> {
        self.mapping.as_ref().expect("live mapping").copy(bytes)
    }
    pub fn checksum(&self) -> u64 {
        self.mapping.as_ref().expect("live mapping").checksum()
    }
    pub fn finish(mut self) -> Result<(), Fault> {
        self.mapping = None;
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        let mut header = [0; 16];
        header[..8].copy_from_slice(b"PLRET001");
        header[8..].copy_from_slice(&self.receiver.generation.to_be_bytes());
        if let Err(error) = channel::write(&self.receiver.socket, &header) {
            self.observation.fail(&error.to_string());
            return Err(error);
        }
        self.returned = true;
        self.observation.finish();
        Ok(())
    }
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.mapping = None;
        if !self.returned {
            self.observation
                .cancel("shared lease dropped without return");
            self.receiver.close();
        }
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.close();
    }
}
