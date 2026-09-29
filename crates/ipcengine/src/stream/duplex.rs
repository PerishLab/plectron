use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

pub struct Duplex(Option<UnixStream>);
impl Duplex {
    pub fn pair() -> std::io::Result<(Self, Self)> {
        let (left, right) = UnixStream::pair()?;
        left.set_nonblocking(true)?;
        right.set_nonblocking(true)?;
        Ok((Self(Some(left)), Self(Some(right))))
    }
    pub fn finish(&self) -> std::io::Result<()> {
        self.socket()?.shutdown(Shutdown::Write)
    }
    pub fn cancel(&mut self) {
        self.0 = None;
    }
    fn socket(&self) -> std::io::Result<&UnixStream> {
        self.0.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "stream was cancelled")
        })
    }
}
impl Read for Duplex {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.socket()?.read(bytes)
    }
}
impl Write for Duplex {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.socket()?.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket()?.flush()
    }
}
