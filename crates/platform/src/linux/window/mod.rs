mod frame;
mod present;

use super::{Error, connection::Connection, error, viewport::View};
use crate::shared::{Event, Image, Window as Options};
use present::{Present, Target};
use std::rc::Rc;
use x11rb::connection::Connection as _;
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, PropMode, Window as Xid,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

pub(super) struct Window {
    connection: Rc<Connection>,
    pub xid: Xid,
    image: Option<Image>,
    frame: Option<graphics::shared::Frame>,
    view: View,
    present: Present,
    pub(super) closing: bool,
    destroyed: bool,
}

impl Window {
    pub fn new(connection: Rc<Connection>, options: &Options) -> Result<Self, Error> {
        connection.check()?;
        options.extent.area().map_err(error)?;
        let mut view = View::new(connection.dpi);
        let (width, height) = view.initialize(options.extent)?;
        let xid = connection.conn.generate_id().map_err(error)?;
        let gc = connection.conn.generate_id().map_err(error)?;
        let aux = CreateWindowAux::new()
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::FOCUS_CHANGE,
            )
            .background_pixel(connection.black)
            .border_pixel(connection.black)
            .colormap(connection.colormap);
        connection
            .conn
            .create_window(
                connection.depth,
                xid,
                connection.root,
                0,
                0,
                width,
                height,
                0,
                WindowClass::INPUT_OUTPUT,
                connection.visual,
                &aux,
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .conn
            .change_property32(
                PropMode::REPLACE,
                xid,
                connection.atoms.protocols,
                AtomEnum::ATOM,
                &[connection.atoms.delete],
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .conn
            .create_gc(
                gc,
                xid,
                &CreateGCAux::new()
                    .foreground(connection.black)
                    .graphics_exposures(0),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        let mut window = Self {
            connection: connection.clone(),
            xid,
            image: None,
            frame: None,
            view,
            present: Present::new(
                connection,
                Target {
                    window: xid,
                    gc,
                    width,
                    height,
                },
            )?,
            closing: false,
            destroyed: false,
        };
        window.title(&options.title)?;
        Ok(window)
    }

    fn check(&self) -> Result<(), Error> {
        self.connection.check()?;
        if self.destroyed {
            return Err(Error("native window was destroyed".into()));
        }
        Ok(())
    }

    pub fn show(&mut self) -> Result<(), Error> {
        self.check()?;
        self.connection
            .conn
            .map_window(self.xid)
            .map_err(error)?
            .check()
            .map_err(error)?;
        self.connection.flush()
    }

    pub fn title(&mut self, title: &str) -> Result<(), Error> {
        self.check()?;
        if title.contains('\0') {
            return Err(Error("native window title contains NUL".into()));
        }
        let bytes = title.as_bytes();
        self.connection
            .conn
            .change_property8(
                PropMode::REPLACE,
                self.xid,
                self.connection.atoms.net,
                self.connection.atoms.utf8,
                bytes,
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        self.connection
            .conn
            .change_property8(
                PropMode::REPLACE,
                self.xid,
                self.connection.atoms.name,
                AtomEnum::STRING,
                bytes,
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        self.connection.flush()
    }

    pub fn present(&mut self, image: Image) -> Result<(), Error> {
        self.install(image, None)
    }

    fn install(
        &mut self,
        image: Image,
        frame: Option<graphics::shared::Frame>,
    ) -> Result<(), Error> {
        self.check()?;
        if image.extent.area().map_err(error)? != image.pixels.len() {
            return Err(Error("native image dimensions disagree with pixels".into()));
        }
        self.frame = frame;
        self.image = Some(image);
        self.present.blit(self.image.as_ref())
    }

    pub fn dispatch(&mut self, event: &XEvent) -> Result<(), Error> {
        match event {
            XEvent::Expose(expose) if expose.window == self.xid && expose.count == 0 => {
                self.present.blit(self.image.as_ref())
            }
            XEvent::ConfigureNotify(configure) if configure.window == self.xid => {
                self.view.configure(configure.width, configure.height)?;
                self.present.resize(configure.width, configure.height)?;
                Ok(())
            }
            XEvent::ClientMessage(message)
                if message.window == self.xid
                    && message.type_ == self.connection.atoms.protocols
                    && message.data.as_data32().first() == Some(&self.connection.atoms.delete) =>
            {
                self.closing = true;
                Ok(())
            }
            XEvent::DestroyNotify(destroy) if destroy.window == self.xid => {
                self.destroyed = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn input(&mut self) -> Vec<Event> {
        self.view.drain()
    }

    pub fn pending(&self) -> bool {
        self.view.pending()
    }

    pub fn closing(&mut self) -> bool {
        std::mem::take(&mut self.closing)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.destroyed {
            let _ = self.connection.conn.destroy_window(self.xid);
            let _ = self.connection.conn.flush();
            self.destroyed = true;
        }
    }
}

#[cfg(test)]
mod pixels;
