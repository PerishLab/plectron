use super::{Effect, Queue};
use crate::linux::Error;
use crate::shared::{Button, Event, Phase, Point};
use x11rb::protocol::xproto::{ButtonPressEvent, ButtonReleaseEvent, MotionNotifyEvent};

impl Queue {
    pub(super) fn press(&self, event: &ButtonPressEvent) -> Result<Effect, Error> {
        match event.detail {
            1 => {
                self.captured.set(true);
                self.push(Event::Pointer {
                    position: self.point(event.event_x, event.event_y),
                    phase: Phase::Down(Button::Primary),
                })?;
                Ok(Effect::Focus(event.time))
            }
            4 | 5 => {
                if let Some(position) = self.point(event.event_x, event.event_y) {
                    self.push(Event::Wheel {
                        position,
                        delta: if event.detail == 4 { -120 } else { 120 },
                    })?;
                }
                Ok(Effect::Idle)
            }
            _ => Ok(Effect::Idle),
        }
    }

    pub(super) fn release(
        &self,
        event: &ButtonReleaseEvent,
        extent: (u16, u16),
    ) -> Result<Effect, Error> {
        if event.detail != 1 {
            return Ok(Effect::Idle);
        }
        let phase = if inside(event.event_x, event.event_y, extent) {
            Phase::Up(Button::Primary)
        } else {
            Phase::Cancel
        };
        self.push(Event::Pointer {
            position: self.point(event.event_x, event.event_y),
            phase,
        })?;
        self.captured.set(false);
        Ok(Effect::Idle)
    }

    pub(super) fn motion(&self, event: &MotionNotifyEvent) -> Result<Effect, Error> {
        self.push(Event::Pointer {
            position: self.point(event.event_x, event.event_y),
            phase: Phase::Move,
        })?;
        Ok(Effect::Idle)
    }

    fn point(&self, x: i16, y: i16) -> Option<Point> {
        let dpi = u64::from(self.density.get().unwrap_or(96));
        Some(Point {
            x: (u64::try_from(x).ok()? * 96 / dpi) as usize,
            y: (u64::try_from(y).ok()? * 96 / dpi) as usize,
        })
    }
}

fn inside(x: i16, y: i16, extent: (u16, u16)) -> bool {
    (0..i32::from(extent.0)).contains(&i32::from(x))
        && (0..i32::from(extent.1)).contains(&i32::from(y))
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

    fn at(x: usize, y: usize) -> Option<Point> {
        Some(Point { x, y })
    }

    #[test]
    fn translate() {
        let queue = Queue::default();
        queue.density(192);
        assert_eq!(queue.press(&button(1, 20, 40)).unwrap(), Effect::Focus(7));
        assert!(queue.captured.get());
        assert_eq!(queue.motion(&moved(-4, 8)).unwrap(), Effect::Idle);
        queue.release(&button(1, 30, 50), (100, 100)).unwrap();
        assert!(!queue.captured.get());
        queue.press(&button(1, 2, 2)).unwrap();
        queue.release(&button(1, 100, 50), (100, 100)).unwrap();
        queue.press(&button(4, 10, 10)).unwrap();
        queue.press(&button(5, 10, 10)).unwrap();
        queue.press(&button(5, -1, 10)).unwrap();
        queue.press(&button(3, 10, 10)).unwrap();
        queue.release(&button(3, 10, 10), (100, 100)).unwrap();
        assert_eq!(
            queue.drain(),
            vec![
                Event::Pointer {
                    position: at(10, 20),
                    phase: Phase::Down(Button::Primary),
                },
                Event::Pointer {
                    position: None,
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
                    position: at(50, 25),
                    phase: Phase::Cancel,
                },
                Event::Wheel {
                    position: at(5, 5).unwrap(),
                    delta: -120,
                },
                Event::Wheel {
                    position: at(5, 5).unwrap(),
                    delta: 120,
                },
            ]
        );
    }
}
