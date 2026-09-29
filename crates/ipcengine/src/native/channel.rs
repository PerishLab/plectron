use crate::Fault;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub fn prepare(socket: &UnixStream) -> Result<(), Fault> {
    socket.set_nonblocking(true)?;
    Ok(())
}
pub fn read(mut socket: &UnixStream, mut bytes: &mut [u8]) -> Result<(), Fault> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !bytes.is_empty() {
        match socket.read(bytes) {
            Ok(0) => return Err(Fault("shared channel disconnected".into())),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                super::wait(socket, false, deadline)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
pub fn write(mut socket: &UnixStream, mut bytes: &[u8]) -> Result<(), Fault> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !bytes.is_empty() {
        match socket.write(bytes) {
            Ok(0) => return Err(Fault("shared channel disconnected".into())),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                super::wait(socket, true, deadline)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
