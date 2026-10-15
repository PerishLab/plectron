use super::Window;
use crate::linux::{Error, error, input::Effect};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{ConnectionExt as _, InputFocus};

impl Window {
    pub fn dispatch(&mut self, event: &XEvent) -> Result<(), Error> {
        match event {
            XEvent::Expose(expose) if expose.window == self.xid && expose.count == 0 => {
                self.present.blit(self.image.as_ref())
            }
            XEvent::ConfigureNotify(configure) if configure.window == self.xid => {
                self.view.configure(configure.width, configure.height)?;
                self.present.resize(configure.width, configure.height)?;
                self.extent = (configure.width, configure.height);
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
                self.queue.close();
                Ok(())
            }
            _ => match self.queue.dispatch(event, self.extent)? {
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

    pub fn remap(&self, first: u8, columns: u8, keysyms: Vec<u32>) {
        self.queue.remap(first, columns, keysyms);
    }
}
