#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::wait;
#[cfg(unix)]
pub use unix::{Listener, Socket};
#[cfg(windows)]
pub(crate) use windows::wait;
#[cfg(windows)]
pub use windows::{Listener, Socket};

#[cfg(windows)]
pub(crate) use windows::{create, prepare};

pub(crate) fn nonblocking(socket: &Socket) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        socket.set_nonblocking(true)
    }
    #[cfg(windows)]
    {
        socket.mode(true)
    }
}
pub(crate) fn duplicate(socket: &Socket) -> std::io::Result<Socket> {
    #[cfg(unix)]
    {
        socket.try_clone()
    }
    #[cfg(windows)]
    {
        socket.duplicate()
    }
}
