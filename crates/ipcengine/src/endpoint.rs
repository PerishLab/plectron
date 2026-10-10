use crate::Fault;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub fn prepare(directory: &Path) -> Result<(), Fault> {
    if let Some(parent) = directory.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => (),
        Err(failure) if failure.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(failure) => return Err(error(failure)),
    }
    let metadata = std::fs::symlink_metadata(directory).map_err(error)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(Fault(
            "IPC directory must be a private directory (mode 0700)".into(),
        ));
    }
    Ok(())
}

pub fn path(directory: &Path, pid: u32) -> PathBuf {
    directory.join(format!("{pid}.sock"))
}
#[cfg(windows)]
pub fn prepare(directory: &Path) -> Result<(), Fault> {
    crate::transport::prepare(directory).map_err(error)
}

pub fn error(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}
