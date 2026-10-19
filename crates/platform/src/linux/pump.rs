use super::{Desktop, Error};
use x11rb::protocol::Event as XEvent;
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
            if let XEvent::MappingNotify(notify) = &event {
                self.remap(notify.request)?;
                continue;
            }
            if let XEvent::PresentCompleteNotify(notify) = &event {
                self.complete(notify);
                continue;
            }
            if let XEvent::PropertyNotify(notify) = &event {
                if notify.window == self.connection.root
                    && notify.atom == self.connection.atoms.resource
                {
                    self.rescale()?;
                }
                continue;
            }
            let xid = match &event {
                XEvent::Expose(event) => event.window,
                XEvent::ConfigureNotify(event) => event.window,
                XEvent::ClientMessage(event) => event.window,
                XEvent::DestroyNotify(event) => event.window,
                XEvent::MapNotify(event) => event.window,
                XEvent::UnmapNotify(event) => event.window,
                XEvent::KeyPress(event) => event.event,
                XEvent::KeyRelease(event) => event.event,
                XEvent::ButtonPress(event) => event.event,
                XEvent::ButtonRelease(event) => event.event,
                XEvent::MotionNotify(event) => event.event,
                XEvent::LeaveNotify(event) => event.event,
                XEvent::FocusIn(event) => event.event,
                XEvent::FocusOut(event) => event.event,
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
