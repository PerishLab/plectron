use crate::Fault;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub struct Rights<'a>(pub &'a UnixStream);
impl Rights<'_> {
    pub fn send(&self, file: &File) -> Result<(), Fault> {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            match self.transmit(file) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    super::wait(self.0, true, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn transmit(&self, file: &File) -> std::io::Result<()> {
        let mut marker = [0x50u8];
        let mut vector = libc::iovec {
            iov_base: marker.as_mut_ptr().cast(),
            iov_len: 1,
        };
        let mut control = [0usize; 8];
        // SAFETY: all msghdr fields are then assigned to live, aligned buffers; the one descriptor is borrowed for this synchronous sendmsg.
        let sent = unsafe {
            let mut message: libc::msghdr = std::mem::zeroed();
            message.msg_iov = &mut vector;
            message.msg_iovlen = 1;
            message.msg_control = control.as_mut_ptr().cast();
            message.msg_controllen = libc::CMSG_SPACE(4) as _;
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(4) as _;
            libc::CMSG_DATA(header)
                .cast::<i32>()
                .write_unaligned(file.as_raw_fd());
            libc::sendmsg(self.0.as_raw_fd(), &message, libc::MSG_NOSIGNAL)
        };
        if sent == 1 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    pub fn receive(&self) -> Result<File, Fault> {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            match self.acquire() {
                Ok(file) => return Ok(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    super::wait(self.0, false, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn acquire(&self) -> std::io::Result<File> {
        let mut marker = [0u8];
        let mut vector = libc::iovec {
            iov_base: marker.as_mut_ptr().cast(),
            iov_len: 1,
        };
        let mut control = [0usize; 8];
        // SAFETY: zero initialization is valid for msghdr; pointers reference writable live buffers of the advertised sizes.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = std::mem::size_of_val(&control) as _;
        // SAFETY: recvmsg owns no Rust references to the data it writes and the socket remains borrowed throughout the call.
        let count = unsafe { libc::recvmsg(self.0.as_raw_fd(), &mut message, 0) };
        if count < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut descriptors = collect(&message);
        if count != 1 || marker[0] != 0x50 || message.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(std::io::Error::other("invalid descriptor envelope"));
        }
        if descriptors.len() != 1 {
            return Err(std::io::Error::other("expected one descriptor"));
        }
        let descriptor = descriptors.pop().expect("one descriptor");
        // SAFETY: this newly received descriptor is live and owned; setting FD_CLOEXEC does not change its data or ownership.
        if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(File::from(descriptor))
    }
}
fn collect(message: &libc::msghdr) -> Vec<OwnedFd> {
    let mut descriptors = Vec::new();
    // SAFETY: only a kernel-populated msghdr reaches this helper. CMSG traversal stays within msg_controllen; every installed descriptor immediately gains one RAII owner.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let length =
                    ((*header).cmsg_len as usize).saturating_sub(libc::CMSG_LEN(0) as usize) / 4;
                for index in 0..length {
                    descriptors.push(OwnedFd::from_raw_fd(
                        libc::CMSG_DATA(header)
                            .cast::<i32>()
                            .add(index)
                            .read_unaligned(),
                    ));
                }
            }
            header = libc::CMSG_NXTHDR(message, header);
        }
    }
    descriptors
}
