use crate::Fault;
pub use std::os::unix::net::UnixStream as Socket;
use std::os::unix::{fs::PermissionsExt, net::UnixListener};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct Listener {
    socket: UnixListener,
    path: PathBuf,
}
impl Listener {
    pub fn bind(path: &Path) -> std::io::Result<Self> {
        let listener = Self {
            socket: UnixListener::bind(path)?,
            path: path.into(),
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        listener.socket.set_nonblocking(true)?;
        Ok(listener)
    }
    pub fn accept(&self) -> std::io::Result<Socket> {
        self.socket.accept().map(|(socket, _)| socket)
    }
    pub(crate) fn wait(&self, deadline: Instant) -> Result<(), Fault> {
        crate::native::wait(&self.socket, false, deadline)
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
pub(crate) fn wait(socket: &Socket, writing: bool, deadline: Instant) -> Result<(), Fault> {
    crate::native::wait(socket, writing, deadline)
}
