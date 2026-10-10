mod delivery;
use crate::{Fault, endpoint};
pub use delivery::Delivery;
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{FileExt, OpenOptionsExt};
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub struct Draft {
    file: File,
    limit: u64,
    length: u64,
}
#[derive(Clone)]
pub struct Artifact {
    file: Arc<File>,
    length: u64,
}
pub struct Reader {
    artifact: Artifact,
    position: u64,
}
impl Draft {
    pub fn create(directory: &Path, limit: u64) -> Result<Self, Fault> {
        let (file, _) = backing(directory)?;
        Ok(Self {
            file,
            limit,
            length: 0,
        })
    }
    pub fn seal(self) -> Result<Artifact, Fault> {
        self.file.sync_all()?;
        Ok(Artifact {
            file: Arc::new(self.file),
            length: self.length,
        })
    }
}
impl Write for Draft {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.length) {
            return Err(std::io::Error::other("file transfer byte budget exhausted"));
        }
        let count = self.file.write(bytes)?;
        self.length += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Artifact {
    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn reader(&self) -> Reader {
        Reader {
            artifact: self.clone(),
            position: 0,
        }
    }
}
impl Read for Reader {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let limit = (self.artifact.length - self.position).min(bytes.len() as u64) as usize;
        let count = read(&self.artifact.file, &mut bytes[..limit], self.position)?;
        self.position += count as u64;
        Ok(count)
    }
}

pub(crate) fn backing(directory: &Path) -> Result<(File, File), Fault> {
    endpoint::prepare(directory)?;
    let id = SEQUENCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Fault("file identities exhausted".into()))?;
    let path = directory.join(format!("{}-{id}.transfer", std::process::id()));
    #[cfg(unix)]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    #[cfg(windows)]
    let file = crate::transport::create(&path, true)?;
    let reader = File::open(&path);
    std::fs::remove_file(path)?;
    let reader = reader?;
    Ok((file, reader))
}

#[cfg(unix)]
fn read(file: &File, bytes: &mut [u8], position: u64) -> std::io::Result<usize> {
    file.read_at(bytes, position)
}
#[cfg(windows)]
fn read(file: &File, bytes: &mut [u8], position: u64) -> std::io::Result<usize> {
    file.seek_read(bytes, position)
}
