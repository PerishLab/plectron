use crate::Fault;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::NonNull;

pub struct Mapping {
    address: NonNull<libc::c_void>,
    length: usize,
    descriptor: File,
    writable: bool,
}
impl Mapping {
    pub fn allocate(directory: &Path, length: usize) -> Result<Self, Fault> {
        if !(1..=16_777_216).contains(&length) {
            return Err(Fault("shared capacity exceeds budget".into()));
        }
        let (writer, reader) = crate::file::backing(directory)?;
        writer.set_len(length as u64)?;
        let mut mapping = Self::map(writer, length, true)?;
        mapping.descriptor = reader;
        Ok(mapping)
    }
    pub fn receive(file: File, length: usize) -> Result<Self, Fault> {
        Self::map(file, length, false)
    }
    fn map(file: File, length: usize, writable: bool) -> Result<Self, Fault> {
        let metadata = file.metadata()?;
        if !(1..=16_777_216).contains(&length)
            || !metadata.is_file()
            || metadata.len() != length as u64
        {
            return Err(Fault("invalid shared mapping extent".into()));
        }
        let protection = libc::PROT_READ | if writable { libc::PROT_WRITE } else { 0 };
        // SAFETY: the owned regular file has the checked nonzero extent; mmap chooses an address, and no descriptor resizing is exposed by this adapter.
        let address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                protection,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        if address == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error().into());
        }
        let Some(address) = NonNull::new(address) else {
            // SAFETY: a successful mapping at address zero still needs its full original extent unmapped before refusing to expose it.
            unsafe {
                libc::munmap(address, length);
            }
            return Err(Fault("null mapping address".into()));
        };
        Ok(Self {
            address,
            length,
            descriptor: file,
            writable,
        })
    }
    pub fn descriptor(&self) -> &File {
        &self.descriptor
    }
    pub fn length(&self) -> usize {
        self.length
    }
    pub fn copy(&self, bytes: &mut [u8]) -> Result<(), Fault> {
        if bytes.len() != self.length {
            return Err(Fault("mapping copy extent mismatch".into()));
        }
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = self.cell(index).load(std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
    pub fn checksum(&self) -> u64 {
        (0..self.length)
            .map(|index| u64::from(self.cell(index).load(std::sync::atomic::Ordering::Relaxed)))
            .sum()
    }
    pub fn fill(&mut self, value: u8) -> Result<(), Fault> {
        if !self.writable {
            return Err(Fault("received mapping is read-only".into()));
        }
        for index in 0..self.length {
            self.cell(index)
                .store(value, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        if !self.writable || bytes.len() != self.length {
            return Err(Fault("mapping write extent mismatch".into()));
        }
        for (index, byte) in bytes.iter().enumerate() {
            self.cell(index)
                .store(*byte, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
    fn cell(&self, index: usize) -> &std::sync::atomic::AtomicU8 {
        // SAFETY: callers bound index by the live mapping length. AtomicU8 has byte alignment and no invalid bit patterns; atomic access avoids exposing aliased Rust byte slices across mappings.
        unsafe {
            &*self
                .address
                .as_ptr()
                .cast::<std::sync::atomic::AtomicU8>()
                .add(index)
        }
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: this object uniquely owns the successful mapping and its original length; all local byte borrows end before Drop.
        unsafe {
            libc::munmap(self.address.as_ptr(), self.length);
        }
    }
}
