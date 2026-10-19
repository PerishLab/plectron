use super::{Effect, Queue};
use crate::linux::Error;
use crate::shared::{Button, Event, LINE, Phase, Point};
use x11rb::protocol::xproto::{
    ButtonPressEvent, ButtonReleaseEvent, EnterNotifyEvent, MotionNotifyEvent, NotifyMode,
};

impl Queue {
    pub(super) fn press(&self, event: &ButtonPressEvent) -> Result<Effect, Error> {
        match event.detail {
            1 => {
                self.captured.set(true);
                self.push(Event::Pointer {
                    position: Some(self.point(event.event_x, event.event_y)),
                    phase: Phase::Down(Button::Primary),
                })?;
                Ok(Effect::Focus(event.time))
            }
            4 | 5 => {
                let delta = if event.detail == 4 { -LINE } else { LINE };
                let position = self.point(event.event_x, event.event_y);
                self.push(Event::Wheel { position, delta })?;
                Ok(Effect::Idle)
            }
            _ => Ok(Effect::Idle),
        }
    }

    pub(super) fn release(&self, event: &ButtonReleaseEvent) -> Result<Effect, Error> {
        if event.detail != 1 || !self.captured.replace(false) {
            return Ok(Effect::Idle);
        }
        self.push(Event::Pointer {
            position: Some(self.point(event.event_x, event.event_y)),
            phase: Phase::Up(Button::Primary),
        })?;
        self.exit()
    }

    pub(super) fn motion(&self, event: &MotionNotifyEvent) -> Result<Effect, Error> {
        self.push(Event::Pointer {
            position: Some(self.point(event.event_x, event.event_y)),
            phase: Phase::Move,
        })?;
        Ok(Effect::Idle)
    }

    pub(super) fn enter(&self, event: &EnterNotifyEvent) -> Result<Effect, Error> {
        self.outside.set(false);
        self.left.set(false);
        self.push(Event::Pointer {
            position: Some(self.point(event.event_x, event.event_y)),
            phase: Phase::Move,
        })?;
        Ok(Effect::Idle)
    }

    pub(super) fn leave(&self, mode: NotifyMode) -> Result<Effect, Error> {
        self.outside.set(true);
        if mode == NotifyMode::GRAB {
            return self.cancel();
        }
        self.exit()
    }

    pub(super) fn exit(&self) -> Result<Effect, Error> {
        if self.captured.get() || !self.outside.get() || self.left.replace(true) {
            return Ok(Effect::Idle);
        }
        self.push(Event::Pointer {
            position: None,
            phase: Phase::Move,
        })?;
        Ok(Effect::Idle)
    }

    fn point(&self, x: i16, y: i16) -> Point {
        let dpi = i64::from(self.density.get().unwrap_or(96));
        Point {
            x: (i64::from(x) * 96).div_euclid(dpi),
            y: (i64::from(y) * 96).div_euclid(dpi),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::protocol::xproto::KeyButMask;

    fn button(detail: u8, x: i16, y: i16) -> ButtonPressEvent {
        ButtonPressEvent {
            response_type: 4,
            detail,
            sequence: 0,
            time: 7,
            root: 1,
            event: 2,
            child: 0,
            root_x: x,
            root_y: y,
            event_x: x,
            event_y: y,
            state: KeyButMask::default(),
            same_screen: true,
        }
    }

    fn moved(x: i16, y: i16) -> MotionNotifyEvent {
        MotionNotifyEvent {
            response_type: 6,
            detail: x11rb::protocol::xproto::Motion::NORMAL,
            sequence: 1,
            time: 9,
            root: 1,
            event: 2,
            child: 0,
            root_x: 0,
            root_y: 0,
            event_x: x,
            event_y: y,
            state: KeyButMask::BUTTON1,
            same_screen: true,
        }
    }

    fn at(x: i64, y: i64) -> Option<Point> {
        Some(Point { x, y })
    }

    #[test]
    fn translate() {
        let queue = Queue::default();
        queue.density(192);
        assert_eq!(queue.press(&button(1, 20, 40)).unwrap(), Effect::Focus(7));
        assert!(queue.captured.get());
        assert_eq!(queue.motion(&moved(-4, 8)).unwrap(), Effect::Idle);
        queue.release(&button(1, 30, 50)).unwrap();
        assert!(!queue.captured.get());
        queue.release(&button(1, 30, 50)).unwrap();
        queue.press(&button(1, 2, 2)).unwrap();
        assert_eq!(queue.motion(&moved(-41, 300)).unwrap(), Effect::Idle);
        queue.release(&button(1, -41, 300)).unwrap();
        queue.press(&button(4, 10, 10)).unwrap();
        queue.press(&button(5, 10, 10)).unwrap();
        queue.press(&button(5, -1, 10)).unwrap();
        queue.press(&button(3, 10, 10)).unwrap();
        queue.release(&button(3, 10, 10)).unwrap();
        assert_eq!(
            queue.drain(),
            vec![
                Event::Pointer {
                    position: at(10, 20),
                    phase: Phase::Down(Button::Primary),
                },
                Event::Pointer {
                    position: at(-2, 4),
                    phase: Phase::Move,
                },
                Event::Pointer {
                    position: at(15, 25),
                    phase: Phase::Up(Button::Primary),
                },
                Event::Pointer {
                    position: at(1, 1),
                    phase: Phase::Down(Button::Primary),
                },
                Event::Pointer {
                    position: at(-21, 150),
                    phase: Phase::Move,
                },
                Event::Pointer {
                    position: at(-21, 150),
                    phase: Phase::Up(Button::Primary),
                },
                Event::Wheel {
                    position: at(5, 5).unwrap(),
                    delta: -120,
                },
                Event::Wheel {
                    position: at(5, 5).unwrap(),
                    delta: 120,
                },
                Event::Wheel {
                    position: at(-1, 5).unwrap(),
                    delta: 120,
                },
            ]
        );
    }
}

#[cfg(test)]
#[path = "../../../tests/linux/crossing.rs"]
mod crossing;
