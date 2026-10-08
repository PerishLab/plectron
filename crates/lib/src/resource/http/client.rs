use super::error;
use crate::contract::Fault;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use webengine::resource::{Cancellation, Request, Response, Url};

const STARTUP: Duration = Duration::from_secs(10);
const IDLE: Duration = Duration::from_secs(2);

pub(in crate::resource) fn fetch(
    request: &Request,
    cancellation: &Cancellation,
    accept: Option<(&str, usize)>,
) -> Result<Response, Fault> {
    let url = &request.url;
    if url.scheme() != "http" {
        return Err(Fault("HTTP transport requires http scheme".into()));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Fault("HTTP host is absent".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| Fault("HTTP port is absent".into()))?;
    cancellation.check()?;
    let address = (host.trim_matches(['[', ']']), port)
        .to_socket_addrs()
        .map_err(error)?
        .next()
        .ok_or_else(|| Fault("HTTP host has no address".into()))?;
    let authority = format!("{host}:{port}");
    send(
        address,
        (&authority, &path(url)),
        (
            cancellation,
            accept.map_or(request.kind.limit(), |(_, limit)| limit),
        ),
        accept.map(|(mime, _)| mime),
    )
}
pub(in crate::resource) fn path(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().into(),
    }
}
pub(super) fn send(
    address: SocketAddr,
    target: (&str, &str),
    control: (&Cancellation, usize),
    accept: Option<&str>,
) -> Result<Response, Fault> {
    let (cancellation, limit) = control;
    if target.0.contains(['\r', '\n', ' ']) || target.1.contains(['\r', '\n', ' ']) {
        return Err(Fault("invalid HTTP request target".into()));
    }
    cancellation.check()?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(error)?;
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        target.1, target.0
    )
    .map_err(error)?;
    if let Some(mime) = accept {
        write!(stream, "Accept: {mime}\r\n").map_err(error)?;
    }
    stream.write_all(b"\r\n").map_err(error)?;
    stream.set_nonblocking(true).map_err(error)?;
    let mut input = Input {
        stream: std::io::BufReader::new(stream),
        limit,
        deadline: Instant::now() + STARTUP,
        cancellation,
    };
    let header = input.line(b"\r\n\r\n", 8192)?;
    let header = String::from_utf8(header).map_err(error)?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|status| (200..600).contains(status))
        .ok_or_else(|| Fault("unsupported HTTP status".into()))?;
    let mime = super::field(&header, "content-type")
        .unwrap_or("application/octet-stream")
        .into();
    let body = if status == 204 || status == 304 {
        Vec::new()
    } else if let Ok(encoding) = super::field(&header, "transfer-encoding") {
        if !encoding.eq_ignore_ascii_case("chunked") {
            return Err(Fault("unsupported HTTP transfer encoding".into()));
        }
        input.chunks()?
    } else if let Ok(length) = super::field(&header, "content-length") {
        let length = length.parse::<usize>().map_err(error)?;
        input.bytes(length)?
    } else {
        input.rest()?
    };
    Ok(Response { status, mime, body })
}
struct Input<'a> {
    stream: std::io::BufReader<TcpStream>,
    limit: usize,
    deadline: Instant,
    cancellation: &'a Cancellation,
}
impl Input<'_> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Fault> {
        loop {
            self.cancellation.check()?;
            if Instant::now() >= self.deadline {
                return Err(Fault("HTTP response deadline exceeded".into()));
            }
            match self.stream.read(output) {
                Ok(read) => {
                    self.deadline = Instant::now() + IDLE;
                    return Ok(read);
                }
                Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(fault) if fault.kind() == std::io::ErrorKind::Interrupted => (),
                Err(fault) => return Err(error(fault)),
            }
        }
    }
    fn byte(&mut self) -> Result<Option<u8>, Fault> {
        let mut byte = [0];
        self.read(&mut byte)
            .map(|read| (read != 0).then_some(byte[0]))
    }
    fn line(&mut self, end: &[u8], limit: usize) -> Result<Vec<u8>, Fault> {
        let mut bytes = Vec::new();
        while !bytes.ends_with(end) {
            if bytes.len() >= limit {
                return Err(Fault("HTTP header budget exhausted".into()));
            }
            bytes.push(
                self.byte()?
                    .ok_or_else(|| Fault("incomplete HTTP response".into()))?,
            );
        }
        Ok(bytes)
    }
    fn bytes(&mut self, length: usize) -> Result<Vec<u8>, Fault> {
        if length > self.limit {
            return Err(Fault("response byte budget exhausted".into()));
        }
        let mut bytes = vec![0; length];
        let mut offset = 0;
        while offset < length {
            let read = self.read(&mut bytes[offset..])?;
            if read == 0 {
                return Err(Fault("incomplete HTTP body".into()));
            }
            offset += read;
        }
        Ok(bytes)
    }
    fn rest(&mut self) -> Result<Vec<u8>, Fault> {
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let available = (self.limit - bytes.len() + 1).min(buffer.len());
            let read = self.read(&mut buffer[..available])?;
            if read == 0 {
                return Ok(bytes);
            }
            if bytes.len() + read > self.limit {
                return Err(Fault("response byte budget exhausted".into()));
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    fn chunks(&mut self) -> Result<Vec<u8>, Fault> {
        let mut body = Vec::new();
        loop {
            let line = String::from_utf8(self.line(b"\r\n", 128)?).map_err(error)?;
            let length = usize::from_str_radix(line.trim(), 16).map_err(error)?;
            if length == 0 {
                if self.line(b"\r\n", 8192)? != b"\r\n" {
                    return Err(Fault("HTTP trailers are unsupported".into()));
                }
                return Ok(body);
            }
            if length > self.limit - body.len() {
                return Err(Fault("response byte budget exhausted".into()));
            }
            body.extend(self.bytes(length)?);
            if self.bytes(2)? != b"\r\n" {
                return Err(Fault("invalid HTTP chunk ending".into()));
            }
        }
    }
}
