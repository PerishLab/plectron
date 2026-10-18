use super::{Error, error};
use graphics::shared::Pacing;
use std::{
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

pub(super) struct Wake(Arc<OwnedFd>);

impl Wake {
    pub fn new() -> Result<Self, Error> {
        // SAFETY: eventfd creates an unnamed counter with the given flags; a non-negative return is a uniquely owned descriptor.
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if fd < 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: eventfd succeeded and transferred ownership of this descriptor.
        Ok(Self(Arc::new(unsafe { OwnedFd::from_raw_fd(fd) })))
    }

    pub fn notifier(&self) -> shared::Notify {
        let fd = self.0.clone();
        shared::Notify::new(move || {
            let value = 1u64;
            // SAFETY: the Arc retains the eventfd across notifications; a full counter returning EAGAIN is ignored.
            let result = unsafe {
                libc::write(
                    fd.as_raw_fd(),
                    (&value as *const u64).cast(),
                    std::mem::size_of_val(&value),
                )
            };
            if result < 0 {
                let fault = std::io::Error::last_os_error();
                if fault.kind() != std::io::ErrorKind::WouldBlock {
                    let _ = fault;
                }
            }
        })
    }

    pub fn wait(&self, xfd: i32, buffered: bool, pacing: Pacing) -> Result<(), Error> {
        if buffered {
            return Ok(());
        }
        let timeout = match pacing {
            Pacing::Idle => -1,
            Pacing::Continuous => 16,
            Pacing::At(deadline) => deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128) as i32,
        };
        let mut descriptors = [
            libc::pollfd {
                fd: xfd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.0.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: both descriptors stay live for this poll; the array length matches the two initialized entries.
        let result = unsafe { libc::poll(descriptors.as_mut_ptr(), 2, timeout) };
        if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(error(std::io::Error::last_os_error()));
        }
        if descriptors[1].revents & libc::POLLIN != 0 {
            let mut value = 0u64;
            // SAFETY: the eventfd is readable; one read clears the level-triggered counter.
            let cleared = unsafe {
                libc::read(
                    self.0.as_raw_fd(),
                    (&mut value as *mut u64).cast(),
                    std::mem::size_of_val(&value),
                )
            };
            if cleared < 0 {
                let fault = std::io::Error::last_os_error();
                if fault.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(error(fault));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn monotonic() -> u64 {
    let mut clock = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one timespec into the valid, exclusively borrowed record.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut clock) } != 0 {
        return 0;
    }
    let seconds = u64::try_from(clock.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(clock.tv_nsec).unwrap_or(0);
    seconds
        .saturating_mul(1_000_000)
        .saturating_add(nanos / 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn wake() {
        let events = Wake::new().unwrap();
        let notifier = events.notifier();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            notifier.notify();
        });
        let started = Instant::now();
        events.wait(-1, false, Pacing::Idle).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        worker.join().unwrap();
        let started = Instant::now();
        events
            .wait(-1, false, Pacing::At(started + Duration::from_millis(20)))
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(15));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
