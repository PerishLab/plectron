use crate::Fault;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Connection {
    pub(super) reader: BufReader<UnixStream>,
    pub(super) poisoned: bool,
}
impl Connection {
    pub fn new(stream: UnixStream) -> Result<Self, Fault> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            reader: BufReader::new(stream),
            poisoned: false,
        })
    }
    pub fn connect(path: &Path) -> Result<Self, Fault> {
        Self::new(UnixStream::connect(path)?)
    }
    pub fn read<T: DeserializeOwned>(
        &mut self,
        limit: usize,
        timeout: Duration,
    ) -> Result<T, Fault> {
        let deadline = Instant::now() + timeout;
        let mut bytes = Vec::new();
        loop {
            if Instant::now() >= deadline {
                return Err(Fault("IPC read deadline exceeded".into()));
            }
            match self.piece(&mut bytes, limit) {
                Ok(true) => return Ok(serde_json::from_slice(&bytes)?),
                Ok(false) => (),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    self.wait(false, deadline)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn piece(&mut self, bytes: &mut Vec<u8>, limit: usize) -> std::io::Result<bool> {
        let buffer = self.reader.fill_buf()?;
        if buffer.is_empty() {
            return Err(std::io::Error::other("IPC frame ended before newline"));
        }
        let end = buffer.iter().position(|byte| *byte == b'\n');
        let count = end.unwrap_or(buffer.len());
        if count > limit.saturating_sub(bytes.len()) {
            return Err(std::io::Error::other("IPC frame exceeds byte budget"));
        }
        bytes.extend_from_slice(&buffer[..count]);
        self.reader.consume(count + usize::from(end.is_some()));
        Ok(end.is_some())
    }
    pub fn write<T: Serialize>(&mut self, value: &T, limit: usize) -> Result<(), Fault> {
        if self.poisoned {
            return Err(Fault(
                "IPC connection failed during a previous write".into(),
            ));
        }
        let mut output = Budget {
            bytes: Vec::new(),
            limit,
        };
        serde_json::to_writer(&mut output, value)?;
        output.bytes.push(b'\n');
        self.transmit(&output.bytes)
    }
}
struct Budget {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("IPC frame exceeds byte budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
