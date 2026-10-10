use super::SEQUENCE;
use crate::Fault;
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

pub struct Delivery {
    file: File,
    temporary: PathBuf,
    target: PathBuf,
    remaining: u64,
}
impl Delivery {
    pub fn new(target: &Path, length: u64) -> Result<Self, Fault> {
        let id = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Fault("delivery identities exhausted".into()))?;
        let temporary = target.with_extension(format!("{}-{id}.partial", std::process::id()));
        #[cfg(unix)]
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        #[cfg(windows)]
        let file = crate::transport::create(&temporary, false)?;
        Ok(Self {
            file,
            temporary,
            target: target.into(),
            remaining: length,
        })
    }
    pub fn finish(self) -> Result<(), Fault> {
        if self.remaining != 0 {
            return Err(Fault("file delivery is incomplete".into()));
        }
        self.file.sync_all()?;
        std::fs::hard_link(&self.temporary, &self.target)?;
        Ok(())
    }
}
impl Write for Delivery {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(std::io::Error::other("file delivery exceeds declared size"));
        }
        let count = self.file.write(bytes)?;
        self.remaining -= count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Drop for Delivery {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
    }
}
