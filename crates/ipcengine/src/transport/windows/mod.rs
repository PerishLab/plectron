#[allow(unsafe_code)]
mod polling;
#[allow(unsafe_code)]
mod storage;
#[allow(unsafe_code)]
mod token;
use crate::Fault;
use interprocess::{
    ConnectWaitMode,
    os::windows::named_pipe::{PipeListener, PipeListenerOptions, PipeStream, pipe_mode::Bytes},
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Write},
    net::Shutdown,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub(crate) use storage::{create, prepare};
pub(crate) use token::descriptor;

type Pipe = PipeStream<Bytes, Bytes>;
pub struct Socket(Arc<Link>);
struct Link {
    pipe: Pipe,
    stopped: AtomicBool,
    written: AtomicBool,
}
pub struct Listener {
    pipe: PipeListener<Bytes, Bytes>,
    _marker: File,
}
impl Listener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        storage::prepare(directory(path)?)?;
        if process(path)? != std::process::id() {
            return Err(io::Error::other(
                "IPC endpoint must name its owning process",
            ));
        }
        let mut marker = create(path, true)?;
        let instance = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        write!(marker, "{instance}")?;
        marker.sync_all()?;
        let pipe = PipeListenerOptions::new()
            .path(name(path, instance)?)
            .nonblocking(true)
            .input_buffer_size_hint(65_536)
            .output_buffer_size_hint(65_536)
            .security_descriptor(Some(descriptor()?))
            .create_duplex::<Bytes>()?;
        Ok(Self {
            pipe,
            _marker: marker,
        })
    }
    pub fn accept(&self) -> io::Result<Socket> {
        Ok(Socket::new(self.pipe.accept()?))
    }
    pub(crate) fn wait(&self, deadline: Instant) -> Result<(), Fault> {
        pause(deadline)
    }
}
impl Socket {
    fn new(pipe: Pipe) -> Self {
        Self(Arc::new(Link {
            pipe,
            stopped: AtomicBool::new(false),
            written: AtomicBool::new(false),
        }))
    }
    pub fn connect(path: &Path) -> io::Result<Self> {
        storage::prepare(directory(path)?)?;
        let pid = process(path)?;
        let mut text = String::new();
        File::open(path)?.take(64).read_to_string(&mut text)?;
        let instance = text.parse().map_err(io::Error::other)?;
        let pipe = Pipe::connect_by_path_with_wait_mode(
            name(path, instance)?,
            ConnectWaitMode::Timeout(Duration::from_secs(2)),
        )?;
        if pipe.server_process_id()? != pid {
            pipe.evade_limbo();
            return Err(io::Error::other("IPC endpoint process identity disagrees"));
        }
        pipe.set_nonblocking(true)?;
        Ok(Self::new(pipe))
    }
    pub fn duplicate(&self) -> io::Result<Self> {
        Ok(Self(self.0.clone()))
    }
    pub fn mode(&self, enabled: bool) -> io::Result<()> {
        self.0.pipe.set_nonblocking(enabled)
    }
    pub fn shutdown(&self, mode: Shutdown) -> io::Result<()> {
        if mode != Shutdown::Both {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "named pipes do not support socket half-close",
            ));
        }
        self.0.stopped.store(true, Ordering::Relaxed);
        self.0.pipe.assume_flushed();
        Ok(())
    }
    fn active(&self) -> io::Result<()> {
        if self.0.stopped.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "IPC connection was cancelled",
            ));
        }
        Ok(())
    }
}
impl Read for Socket {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.active()?;
        if bytes.is_empty() {
            return Ok(0);
        }
        match polling::available(&self.0.pipe)? {
            None => return Ok(0),
            Some(0) => return Err(io::ErrorKind::WouldBlock.into()),
            Some(_) => (),
        }
        (&self.0.pipe).read(bytes)
    }
}
impl Write for Socket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.active()?;
        let count = (&self.0.pipe).write(&bytes[..bytes.len().min(65_536)])?;
        if count == 0 && !bytes.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.0.written.store(true, Ordering::Relaxed);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.active()
    }
}
impl Drop for Link {
    fn drop(&mut self) {
        if self.pipe.is_server()
            && self.written.load(Ordering::Relaxed)
            && !self.stopped.load(Ordering::Relaxed)
        {
            let deadline = Instant::now() + Duration::from_secs(2);
            while matches!(polling::available(&self.pipe), Ok(Some(_))) {
                if pause(deadline).is_err() {
                    break;
                }
            }
        }
        self.pipe.assume_flushed();
    }
}
pub(crate) fn wait(socket: &Socket, _: bool, deadline: Instant) -> Result<(), Fault> {
    socket.active()?;
    pause(deadline)
}
fn pause(deadline: Instant) -> Result<(), Fault> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Fault("IPC readiness deadline exceeded".into()));
    }
    std::thread::sleep(remaining.min(Duration::from_millis(1)));
    Ok(())
}
fn process(path: &Path) -> io::Result<u32> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.parse().ok())
        .ok_or_else(|| io::Error::other("IPC endpoint has no process identity"))
}
fn name(path: &Path, instance: u128) -> io::Result<String> {
    let root = directory(path)?;
    let root = std::fs::canonicalize(root)?;
    let mut hash = Sha256::new();
    for unit in root.as_os_str().encode_wide() {
        hash.update(unit.to_le_bytes());
    }
    Ok(format!(
        "\\\\.\\pipe\\plectron-{:x}-{}-{instance}",
        hash.finalize(),
        process(path)?
    ))
}
fn directory(path: &Path) -> io::Result<&Path> {
    path.parent()
        .ok_or_else(|| io::Error::other("IPC endpoint has no directory"))
}
