mod client;
use super::error;
use crate::contract::Fault;
pub(super) use client::{fetch, path};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use webengine::resource::Response;

pub fn head(stream: &mut TcpStream) -> Result<String, Fault> {
    stream.set_nonblocking(true).map_err(error)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        if bytes.len() >= 8192 {
            return Err(Fault("HTTP header budget exhausted".into()));
        }
        if Instant::now() >= deadline {
            return Err(Fault("HTTP header deadline exceeded".into()));
        }
        let mut byte = [0];
        match stream.read_exact(&mut byte) {
            Ok(()) => (),
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(fault) if fault.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(fault) => return Err(error(fault)),
        }
        bytes.push(byte[0]);
    }
    stream.set_nonblocking(false).map_err(error)?;
    String::from_utf8(bytes).map_err(error)
}

fn field<'a>(header: &'a str, name: &str) -> Result<&'a str, Fault> {
    header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .ok_or_else(|| Fault(format!("missing HTTP {name}")))
}

pub fn send(stream: &mut TcpStream, response: Response) -> Result<(), Fault> {
    stream.set_nonblocking(false).map_err(error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(error)?;
    write!(stream, "HTTP/1.1 {} Response\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.status, response.mime, response.body.len()).map_err(error)?;
    stream.write_all(&response.body).map_err(error)
}

pub(super) fn load(
    address: SocketAddr,
    target: (&str, usize),
    cancellation: &webengine::resource::Cancellation,
) -> Result<Response, Fault> {
    client::send(
        address,
        (&address.to_string(), target.0),
        (cancellation, target.1),
    )
}
