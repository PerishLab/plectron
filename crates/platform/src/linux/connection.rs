use super::{Error, error};
use std::cell::RefCell;
use std::os::fd::AsRawFd;
use std::thread::ThreadId;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, ImageOrder, Screen, VisualClass, Visualid, Window,
};
use x11rb::protocol::{Event, shm};
use x11rb::rust_connection::RustConnection;

pub(super) struct Atoms {
    pub protocols: Atom,
    pub delete: Atom,
    pub net: Atom,
    pub utf8: Atom,
    pub name: Atom,
    pub pid: Atom,
}

struct Values {
    visual: Visualid,
    depth: u8,
    root: Window,
    black: u32,
    colormap: u32,
    atoms: Atoms,
    dpi: u32,
    swap: bool,
    shm: bool,
}

pub(super) struct Connection {
    pub conn: RustConnection,
    pub root: Window,
    pub visual: Visualid,
    pub depth: u8,
    pub colormap: u32,
    pub black: u32,
    pub atoms: Atoms,
    pub dpi: u32,
    pub swap: bool,
    pub shm: bool,
    pending: RefCell<Option<Event>>,
    owner: ThreadId,
}

impl Connection {
    pub fn open() -> Result<Self, Error> {
        let (conn, screen) = RustConnection::connect(None).map_err(error)?;
        let values = {
            let setup = conn.setup();
            let info = setup
                .roots
                .get(screen)
                .ok_or_else(|| Error("X11 screen is absent".into()))?;
            let (visual, depth) = select(setup, info)?;
            let swap = setup.image_byte_order != ImageOrder::LSB_FIRST;
            let root = info.root;
            let black = info.black_pixel;
            let colormap = colormap(&conn, info, root, visual)?;
            let atoms = Atoms {
                protocols: intern(&conn, "WM_PROTOCOLS")?,
                delete: intern(&conn, "WM_DELETE_WINDOW")?,
                net: intern(&conn, "_NET_WM_NAME")?,
                utf8: intern(&conn, "UTF8_STRING")?,
                name: AtomEnum::WM_NAME.into(),
                pid: intern(&conn, "_NET_WM_PID")?,
            };
            let shm = match shm::query_version(&conn) {
                Ok(cookie) => cookie
                    .reply()
                    .ok()
                    .is_some_and(|reply| reply.major_version >= 1 && reply.minor_version >= 2),
                Err(_) => false,
            };
            let dpi = x11rb::resource_manager::new_from_default(&conn)
                .ok()
                .and_then(|database| database.get_value("Xft.dpi", "").ok().flatten())
                .unwrap_or(96)
                .clamp(24, 768);
            Values {
                visual,
                depth,
                root,
                black,
                colormap,
                atoms,
                dpi,
                swap,
                shm,
            }
        };
        Ok(Self {
            conn,
            root: values.root,
            visual: values.visual,
            depth: values.depth,
            colormap: values.colormap,
            black: values.black,
            atoms: values.atoms,
            dpi: values.dpi,
            swap: values.swap,
            shm: values.shm,
            pending: RefCell::new(None),
            owner: std::thread::current().id(),
        })
    }

    pub fn check(&self) -> Result<(), Error> {
        if std::thread::current().id() != self.owner {
            return Err(Error(
                "native window operation requires its owner thread".into(),
            ));
        }
        Ok(())
    }

    pub fn fd(&self) -> i32 {
        self.conn.stream().as_raw_fd()
    }

    pub fn flush(&self) -> Result<(), Error> {
        self.conn.flush().map_err(error)
    }

    pub fn buffered(&self) -> Result<bool, Error> {
        if self.pending.borrow().is_some() {
            return Ok(true);
        }
        if let Some(event) = self.conn.poll_for_event().map_err(error)? {
            *self.pending.borrow_mut() = Some(event);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn keymap(&self) -> Result<(u8, u8, Vec<u32>), Error> {
        let setup = self.conn.setup();
        let first = setup.min_keycode;
        let count = setup.max_keycode.saturating_sub(first).saturating_add(1);
        let reply = self
            .conn
            .get_keyboard_mapping(first, count)
            .map_err(error)?
            .reply()
            .map_err(error)?;
        Ok((first, reply.keysyms_per_keycode, reply.keysyms))
    }

    pub fn take(&self) -> Result<Option<Event>, Error> {
        if let Some(event) = self.pending.borrow_mut().take() {
            return Ok(Some(event));
        }
        self.conn.poll_for_event().map_err(error)
    }
}

fn colormap(
    conn: &RustConnection,
    info: &Screen,
    root: Window,
    visual: Visualid,
) -> Result<u32, Error> {
    if visual == info.root_visual {
        return Ok(info.default_colormap);
    }
    let id = conn.generate_id().map_err(error)?;
    conn.create_colormap(
        x11rb::protocol::xproto::ColormapAlloc::NONE,
        id,
        root,
        visual,
    )
    .map_err(error)?
    .check()
    .map_err(error)?;
    Ok(id)
}

fn intern(conn: &RustConnection, name: &str) -> Result<Atom, Error> {
    Ok(conn
        .intern_atom(false, name.as_bytes())
        .map_err(error)?
        .reply()
        .map_err(error)?
        .atom)
}

fn select(
    setup: &x11rb::protocol::xproto::Setup,
    screen: &Screen,
) -> Result<(Visualid, u8), Error> {
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == 24)
        .ok_or_else(|| Error("X11 depth 24 pixmap format is absent".into()))?;
    if format.bits_per_pixel != 32 {
        return Err(Error("X11 depth 24 requires 32 bits per pixel".into()));
    }
    for depth in &screen.allowed_depths {
        if depth.depth != 24 {
            continue;
        }
        for visual in &depth.visuals {
            if !matches!(visual.class, VisualClass::TRUE_COLOR) {
                continue;
            }
            if visual.red_mask != 0xff_0000 {
                continue;
            }
            if visual.green_mask != 0x00_ff00 {
                continue;
            }
            if visual.blue_mask != 0x00_00ff {
                continue;
            }
            return Ok((visual.visual_id, 24));
        }
    }
    Err(Error(
        "X11 depth24 TrueColor visual with standard masks is absent".into(),
    ))
}
