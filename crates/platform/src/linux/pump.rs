use super::{Desktop, Error};
use x11rb::protocol;
use x11rb::protocol::present::CompleteNotifyEvent;
use x11rb::protocol::xproto::Mapping;

impl<E> Desktop<E> {
    fn remap(&mut self, request: Mapping) -> Result<(), Error> {
        if request != Mapping::KEYBOARD {
            return Ok(());
        }
        let (first, columns, keysyms) = self.connection.keymap()?;
        for window in self.windows.values() {
            window.remap(first, columns, keysyms.clone())?;
        }
        Ok(())
    }

    fn rescale(&mut self) -> Result<(), Error> {
        let Some(dpi) = self.connection.reload() else {
            return Ok(());
        };
        for window in self.windows.values_mut() {
            window.rescale(dpi)?;
        }
        Ok(())
    }

    pub(super) fn complete(&self, notify: &CompleteNotifyEvent) -> bool {
        self.xids
            .get(&notify.window)
            .and_then(|unit| self.windows.get(unit))
            .is_some_and(|window| window.timing.observe(notify))
    }

    pub(super) fn pump(&mut self) -> Result<(), Error> {
        for _ in 0..256 {
            let Some(event) = self.connection.take()? else {
                break;
            };
            if let protocol::Event::MappingNotify(notify) = &event {
                self.remap(notify.request)?;
                continue;
            }
            if let protocol::Event::PresentCompleteNotify(notify) = &event {
                self.complete(notify);
                continue;
            }
            if let protocol::Event::PropertyNotify(notify) = &event {
                if notify.window == self.connection.root
                    && notify.atom == self.connection.atoms.resource
                {
                    self.rescale()?;
                }
                continue;
            }
            let xid = match &event {
                protocol::Event::Expose(event) => event.window,
                protocol::Event::ConfigureNotify(event) => event.window,
                protocol::Event::ClientMessage(event) => event.window,
                protocol::Event::DestroyNotify(event) => event.window,
                protocol::Event::MapNotify(event) => event.window,
                protocol::Event::UnmapNotify(event) => event.window,
                protocol::Event::KeyPress(event) => event.event,
                protocol::Event::KeyRelease(event) => event.event,
                protocol::Event::ButtonPress(event) => event.event,
                protocol::Event::ButtonRelease(event) => event.event,
                protocol::Event::MotionNotify(event) => event.event,
                protocol::Event::EnterNotify(event) => event.event,
                protocol::Event::LeaveNotify(event) => event.event,
                protocol::Event::FocusIn(event) => event.event,
                protocol::Event::FocusOut(event) => event.event,
                _ => continue,
            };
            let Some(unit) = self.xids.get(&xid).copied() else {
                continue;
            };
            if let Some(window) = self.windows.get_mut(&unit) {
                window.dispatch(&event)?;
            }
        }
        Ok(())
    }
}
