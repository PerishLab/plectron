use super::{CHUNK, Chunk, Command, Transfer};
use crate::{
    Fault,
    message::Connection,
    observe::{Lease, Monitor},
};
use std::io::Read;
use std::time::{Duration, Instant};
const DATA: u8 = 1;
const END: u8 = 2;
const CLOSED: u8 = 3;
const FAILED: u8 = 4;

fn send(connection: &mut Connection, kind: u8, bytes: &[u8]) -> Result<(), Fault> {
    let mut header = [0; 16];
    header[..4].copy_from_slice(b"PLS2");
    header[4] = kind;
    header[8..].copy_from_slice(&(bytes.len() as u64).to_be_bytes());
    connection.transmit(&header)?;
    connection.transmit(bytes)
}
fn receive(connection: &mut Connection, limit: usize) -> Result<(u8, Vec<u8>), Fault> {
    let mut header = [0; 16];
    connection.receive(&mut header, Duration::from_secs(6))?;
    if &header[..4] != b"PLS2" || header[5..8] != [0; 3] {
        return Err(Fault("invalid binary stream header".into()));
    }
    let length = u64::from_be_bytes(header[8..].try_into().expect("header length"));
    let kind = header[4];
    let budget = if kind == FAILED { 4096 } else { limit };
    if ![DATA, END, CLOSED, FAILED].contains(&kind) || length > budget as u64 {
        return Err(Fault("binary stream frame exceeds contract".into()));
    }
    let mut bytes = vec![0; length as usize];
    connection.receive(&mut bytes, Duration::from_secs(6))?;
    if kind == FAILED {
        return Err(Fault(String::from_utf8_lossy(&bytes).into_owned()));
    }
    Ok((kind, bytes))
}
impl<R: Read> Transfer<R> {
    pub fn serve(mut self, connection: &mut Connection) {
        if let Err(error) = self.exchange(connection) {
            self.observation.fail(&error.to_string());
            let bytes = error.to_string().into_bytes();
            let _ = send(connection, FAILED, &bytes[..bytes.len().min(4096)]);
        }
    }
    fn exchange(&mut self, connection: &mut Connection) -> Result<(), Fault> {
        loop {
            let started = Instant::now();
            let command: Command = connection.read(65_536, Duration::from_secs(6))?;
            self.observation.wait(started);
            match command {
                Command::Read { limit } => {
                    let chunk = self.read(limit)?;
                    payload(connection, &chunk)?;
                }
                Command::Close => {
                    self.close();
                    return send(connection, CLOSED, &[]);
                }
            }
        }
    }
}
pub struct Client<'a> {
    connection: &'a mut Connection,
    remaining: u64,
    observation: Lease,
    returned: bool,
    failed: bool,
}
impl<'a> Client<'a> {
    pub fn new(connection: &'a mut Connection, length: u64, monitor: &Monitor) -> Self {
        Self {
            connection,
            remaining: length,
            observation: monitor.lease(),
            returned: false,
            failed: false,
        }
    }
    pub fn read(&mut self, limit: usize) -> Result<Chunk, Fault> {
        if self.failed {
            return Err(Fault("binary stream has failed".into()));
        }
        let started = Instant::now();
        let result = self.chunk(limit);
        self.observation.wait(started);
        match &result {
            Ok(chunk) => self.observation.bytes(chunk.bytes.len()),
            Err(error) => {
                self.failed = true;
                self.observation.fail(&error.to_string());
                self.connection.abort();
            }
        }
        result
    }
    fn chunk(&mut self, limit: usize) -> Result<Chunk, Fault> {
        if limit == 0 || limit > CHUNK {
            return Err(Fault("invalid stream chunk budget".into()));
        }
        self.connection.write(&Command::Read { limit }, 65_536)?;
        let (kind, bytes) = receive(self.connection, limit)?;
        if ![DATA, END].contains(&kind) || bytes.len() as u64 > self.remaining {
            return Err(Fault("unexpected binary stream response".into()));
        }
        self.remaining -= bytes.len() as u64;
        let eof = kind == END;
        if eof != (self.remaining == 0) || (bytes.is_empty() && !eof) {
            return Err(Fault("binary stream length and EOF disagree".into()));
        }
        Ok(Chunk { bytes, eof })
    }
    pub fn close(mut self) -> Result<(), Fault> {
        let result = self.release();
        if let Err(error) = &result {
            self.observation.fail(&error.to_string());
        }
        result
    }
    fn release(&mut self) -> Result<(), Fault> {
        if self.failed {
            return Err(Fault("binary stream has failed".into()));
        }
        self.connection.write(&Command::Close, 65_536)?;
        let (kind, bytes) = receive(self.connection, 0)?;
        if kind != CLOSED || !bytes.is_empty() {
            return Err(Fault("stream did not close".into()));
        }
        self.returned = true;
        if self.remaining == 0 {
            self.observation.finish();
        } else {
            self.observation.cancel("receiver closed before EOF");
        }
        Ok(())
    }
}

fn payload(connection: &mut Connection, chunk: &Chunk) -> Result<(), Fault> {
    send(connection, if chunk.eof { END } else { DATA }, &chunk.bytes)
}

impl Drop for Client<'_> {
    fn drop(&mut self) {
        if !self.returned {
            self.connection.abort();
        }
    }
}
