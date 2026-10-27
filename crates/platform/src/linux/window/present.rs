use super::super::paint::{Area, Put, clear, put};
use super::super::{Error, connection::Connection, error};
use crate::shared::Image;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr::NonNull;
use std::rc::Rc;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, Drawable, Gcontext, ImageFormat, Window};
use x11rb::protocol::{present, shm};
pub(super) struct Target {
    pub window: Window,
    pub gc: Gcontext,
    pub width: u16,
    pub height: u16,
    pub native: bool,
}
pub(super) struct Present {
    connection: Rc<Connection>,
    window: Window,
    gc: Gcontext,
    width: u16,
    height: u16,
    shm: Option<Segment>,
    pub(super) native: bool,
}
impl Present {
    pub fn new(connection: Rc<Connection>, target: Target) -> Result<Self, Error> {
        let mut present = Self {
            connection,
            window: target.window,
            gc: target.gc,
            width: target.width,
            height: target.height,
            shm: None,
            native: target.native,
        };
        present.rebuild(target.width, target.height)?;
        Ok(present)
    }
    pub fn resize(&mut self, width: u16, height: u16) -> Result<(), Error> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if width == self.width && height == self.height {
            return Ok(());
        }
        self.rebuild(width, height)
    }
    fn rebuild(&mut self, width: u16, height: u16) -> Result<(), Error> {
        self.release();
        self.width = width;
        self.height = height;
        if width == 0 || height == 0 {
            return Ok(());
        }
        if self.connection.shm {
            match attach(&self.connection, width, height) {
                Ok(segment) => self.shm = Some(segment),
                Err(_) => self.shm = None,
            }
        }
        Ok(())
    }
    pub fn blit(&mut self, image: Option<&Image>) -> Result<(), Error> {
        self.paint(Drawable::from(self.window), image)?;
        self.connection.conn.flush().map_err(error)
    }
    pub fn show(&mut self, image: &Image, serial: u32) -> Result<Option<u32>, Error> {
        if self.width == 0 || self.height == 0 {
            return Ok(None);
        }
        if self.native {
            if self.hand(image, serial).is_ok() {
                return Ok(Some(serial));
            }
            self.native = false;
        }
        self.blit(Some(image)).map(|()| None)
    }
    fn hand(&self, image: &Image, serial: u32) -> Result<(), Error> {
        let conn = &self.connection.conn;
        let pixmap = conn.generate_id().map_err(error)?;
        let created = conn
            .create_pixmap(
                self.connection.depth,
                pixmap,
                self.window,
                self.width,
                self.height,
            )
            .map_err(error)?;
        let handed = self
            .paint(Drawable::from(pixmap), Some(image))
            .and_then(|()| created.check().map_err(error))
            .and_then(|()| {
                present::pixmap(
                    conn,
                    self.window,
                    pixmap,
                    serial,
                    x11rb::NONE,
                    x11rb::NONE,
                    0,
                    0,
                    x11rb::NONE,
                    x11rb::NONE,
                    x11rb::NONE,
                    u32::from(u8::from(present::Option::NONE)),
                    0,
                    0,
                    0,
                    &[],
                )
                .map_err(error)?
                .check()
                .map_err(error)
            });
        conn.free_pixmap(pixmap).map_err(error)?.ignore_error();
        conn.flush().map_err(error)?;
        handed
    }
    fn paint(&self, drawable: Drawable, image: Option<&Image>) -> Result<(), Error> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        let conn = &self.connection.conn;
        let area = Area {
            drawable,
            gc: self.gc,
            width: self.width,
            height: self.height,
        };
        match image {
            None => clear(conn, area),
            Some(image) => {
                let width = (image.extent.width as u16).min(self.width);
                let height = (image.extent.height as u16).min(self.height);
                if width < self.width || height < self.height {
                    clear(conn, area)?;
                }
                if width == 0 || height == 0 {
                    return Ok(());
                }
                let extent = Extent {
                    width,
                    height,
                    swap: self.connection.swap,
                };
                if let Some(segment) = self.shm.as_ref() {
                    write(segment, image, extent);
                    shm::put_image(
                        conn,
                        drawable,
                        self.gc,
                        width,
                        height,
                        0,
                        0,
                        width,
                        height,
                        0,
                        0,
                        self.connection.depth,
                        ImageFormat::Z_PIXMAP.into(),
                        false,
                        segment.seg,
                        0,
                    )
                    .map_err(error)?
                    .check()
                    .map_err(error)
                } else {
                    put(
                        conn,
                        Put {
                            drawable,
                            gc: self.gc,
                            image,
                            width,
                            height,
                            swap: self.connection.swap,
                        },
                    )
                }
            }
        }
    }
    fn release(&mut self) {
        if let Some(segment) = self.shm.take() {
            let _ = shm::detach(&self.connection.conn, segment.seg);
            drop(segment);
        }
    }
}
impl Drop for Present {
    fn drop(&mut self) {
        self.release();
    }
}
struct Segment {
    seg: shm::Seg,
    address: NonNull<u8>,
    length: usize,
    _fd: OwnedFd,
}
impl Drop for Segment {
    fn drop(&mut self) {
        // SAFETY: address and length were produced by a successful mmap of this segment and are not used afterward.
        unsafe {
            libc::munmap(self.address.as_ptr().cast(), self.length);
        }
    }
}
fn attach(connection: &Connection, width: u16, height: u16) -> Result<Segment, Error> {
    let length = usize::from(width)
        .checked_mul(usize::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| Error("native shared image exceeds budget".into()))?;
    // SAFETY: memfd_create returns a uniquely owned anonymous file descriptor when successful.
    let fd = unsafe { libc::memfd_create(c"plectron-shm".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    // SAFETY: memfd_create succeeded; ownership transfers into OwnedFd.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    if unsafe { libc::ftruncate(fd.as_raw_fd(), length as libc::off_t) } != 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    // SAFETY: the memfd has the truncated nonzero length; mmap chooses an address with shared read/write mapping.
    let address = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };
    if address == libc::MAP_FAILED {
        return Err(error(std::io::Error::last_os_error()));
    }
    let Some(address) = NonNull::new(address.cast::<u8>()) else {
        // SAFETY: a successful mapping at null still requires munmap of the original extent.
        unsafe {
            libc::munmap(address, length);
        }
        return Err(Error("null shared mapping address".into()));
    };
    let seg = connection.conn.generate_id().map_err(error)?;
    let passed = duplicate(&fd)?;
    shm::attach_fd(&connection.conn, seg, passed, false)
        .map_err(error)?
        .check()
        .map_err(error)?;
    Ok(Segment {
        seg,
        address,
        length,
        _fd: fd,
    })
}
fn duplicate(fd: &OwnedFd) -> Result<OwnedFd, Error> {
    // SAFETY: F_DUPFD_CLOEXEC duplicates a live descriptor into a new owned fd.
    let duplicated = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    // SAFETY: fcntl returned a uniquely owned duplicate descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated as RawFd) })
}
struct Extent {
    width: u16,
    height: u16,
    swap: bool,
}
fn write(segment: &Segment, image: &Image, extent: Extent) {
    let stride = usize::from(extent.width);
    // SAFETY: the mapping covers width*height*4 bytes for this segment; writes stay within that extent.
    let pixels = unsafe {
        std::slice::from_raw_parts_mut(segment.address.as_ptr().cast::<u32>(), segment.length / 4)
    };
    for y in 0..usize::from(extent.height) {
        let source = y * image.extent.width;
        let destination = y * stride;
        for x in 0..stride {
            let value = image.pixels[source + x];
            pixels[destination + x] = if extent.swap {
                value.swap_bytes()
            } else {
                value
            };
        }
    }
}
