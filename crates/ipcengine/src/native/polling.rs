use crate::Fault;
use std::os::fd::{AsFd, AsRawFd};
use std::time::Instant;

pub fn wait(stream: &impl AsFd, writing: bool, deadline: Instant) -> Result<(), Fault> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Fault("IPC readiness deadline exceeded".into()));
        }
        let mut descriptor = libc::pollfd {
            fd: stream.as_fd().as_raw_fd(),
            events: if writing { libc::POLLOUT } else { libc::POLLIN },
            revents: 0,
        };
        let timeout = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        // SAFETY: descriptor is initialized and live for the one-element poll array; the borrowed socket remains open.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout) };
        if result > 0 {
            return Ok(());
        }
        if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(std::io::Error::last_os_error().into());
        }
    }
}
