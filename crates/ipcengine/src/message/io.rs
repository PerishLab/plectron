use super::Connection;
use crate::Fault;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

impl Connection {
    pub(crate) fn abort(&mut self) {
        self.poisoned = true;
        let _ = self.reader.get_ref().shutdown(std::net::Shutdown::Both);
    }
    pub(crate) fn receive(&mut self, bytes: &mut [u8], timeout: Duration) -> Result<(), Fault> {
        let deadline = Instant::now() + timeout;
        let mut offset = 0;
        while offset < bytes.len() {
            match self.reader.read(&mut bytes[offset..]) {
                Ok(0) => return Err(Fault("IPC payload ended prematurely".into())),
                Ok(count) => offset += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    self.wait(false, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(error.into()),
            }
            if Instant::now() >= deadline {
                return Err(Fault("IPC payload deadline exceeded".into()));
            }
        }
        Ok(())
    }
    pub(crate) fn transmit(&mut self, mut bytes: &[u8]) -> Result<(), Fault> {
        if self.poisoned {
            return Err(Fault(
                "IPC connection failed during a previous write".into(),
            ));
        }
        self.poisoned = true;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !bytes.is_empty() {
            match self.reader.get_mut().write(bytes) {
                Ok(0) => return Err(Fault("IPC peer closed".into())),
                Ok(count) => bytes = &bytes[count..],
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    self.wait(true, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(error.into()),
            }
            if Instant::now() >= deadline {
                return Err(Fault("IPC write deadline exceeded".into()));
            }
        }
        self.poisoned = false;
        Ok(())
    }
    pub(super) fn wait(&self, writing: bool, deadline: Instant) -> Result<(), Fault> {
        crate::transport::wait(self.reader.get_ref(), writing, deadline)
    }
}
