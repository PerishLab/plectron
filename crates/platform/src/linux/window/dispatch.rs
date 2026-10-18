use super::Window;
use crate::linux::{Error, error, input::Effect};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus};

impl Window {
    pub fn dispatch(&mut self, event: &XEvent) -> Result<(), Error> {
        match event {
            XEvent::Expose(expose) if expose.window == self.xid && expose.count == 0 => {
                let result = self.present.blit(self.image.as_ref());
                self.settle(result)
            }
            XEvent::ConfigureNotify(configure) if configure.window == self.xid => {
                self.view
                    .configure(configure.width, configure.height, &self.queue)?;
                self.extent = (configure.width, configure.height);
                let result = self.present.resize(configure.width, configure.height);
                self.settle(result)
            }
            XEvent::ClientMessage(message)
                if message.window == self.xid
                    && message.type_ == self.connection.atoms.protocols
                    && message.data.as_data32().first() == Some(&self.connection.atoms.ping) =>
            {
                self.pong(message)
            }
            XEvent::ClientMessage(message)
                if message.window == self.xid
                    && message.type_ == self.connection.atoms.protocols
                    && message.data.as_data32().first() == Some(&self.connection.atoms.delete) =>
            {
                self.queue.close();
                self.closing = true;
                Ok(())
            }
            XEvent::DestroyNotify(destroy) if destroy.window == self.xid => {
                self.destroyed = true;
                self.queue.close();
                self.closing = true;
                Ok(())
            }
            _ => match self.queue.dispatch(event)? {
                Effect::Idle => Ok(()),
                Effect::Focus(time) => {
                    self.connection
                        .conn
                        .set_input_focus(InputFocus::PARENT, self.xid, time)
                        .map_err(error)?
                        .ignore_error();
                    Ok(())
                }
            },
        }
    }

    fn pong(&self, message: &ClientMessageEvent) -> Result<(), Error> {
        let root = self.connection.root;
        let reply = ClientMessageEvent {
            window: root,
            ..*message
        };
        self.connection
            .conn
            .send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
                reply,
            )
            .map_err(error)?
            .ignore_error();
        self.connection.flush()
    }

    pub fn rescale(&mut self, dpi: u32) -> Result<(), Error> {
        if self.destroyed {
            return Ok(());
        }
        self.view.rescale(dpi, self.extent, &self.queue)
    }

    pub fn remap(&self, first: u8, columns: u8, keysyms: Vec<u32>) -> Result<(), Error> {
        self.queue.remap(first, columns, keysyms)
    }
}
