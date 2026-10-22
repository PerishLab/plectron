use super::{Canvas, Error, window};
use crate::shared::{Button, Event, Phase, Point, Receipt, Signal, Unit};

impl Canvas {
    pub(super) fn advance(
        &mut self,
        target: (Unit, bool),
        signals: &mut Vec<Signal>,
        receipts: &mut Vec<Receipt<Error>>,
    ) -> Result<(), Error> {
        let (unit, accelerated) = target;
        let mut result = self.poll(accelerated).map(|()| None);
        let mut pointer = Vec::new();
        for event in self.window.drain()? {
            match event {
                window::Event::Close => result = Ok(Some(Event::Close)),
                window::Event::Move { x, y } => self.pointer.moved(point(x, y), &mut pointer),
                window::Event::Down { x, y } => self.pointer.down(point(x, y), &mut pointer),
                window::Event::Up { x, y, outside } => {
                    self.pointer.up(point(x, y), outside, &mut pointer)
                }
                window::Event::Enter { x, y } => self.pointer.enter(point(x, y), &mut pointer),
                window::Event::Exit => self.pointer.exit(&mut pointer),
                window::Event::Wheel { x, y, delta } => signals.extend(wheel(unit, x, y, delta)),
                window::Event::Key {
                    code,
                    repeat,
                    flags,
                } => self.input.key(code, repeat, flags),
            }
            signals.extend(pointer.drain(..).map(|event| Signal { unit, event }));
        }
        self.input.drain(unit, signals)?;
        self.ime.drain(unit, signals)?;
        if !self.ime.active() {
            self.input.release();
            self.ime.cancel();
        }
        if let Ok(Some(event)) = &result {
            signals.push(Signal {
                unit,
                event: event.clone(),
            });
        }
        if let Some(frame) = self.pending.take() {
            let result = result.and_then(|event| {
                if event == Some(Event::Close) {
                    Err(Error("window closed before presentation".into()))
                } else {
                    Ok(())
                }
            });
            receipts.push(Receipt {
                produced: Some(std::time::Instant::now()),
                visible: None,
                drops: 0,
                unit,
                frame,
                result,
            });
        } else {
            result?;
        }
        Ok(())
    }
}

fn wheel(unit: Unit, x: f64, y: f64, delta: f64) -> Option<Signal> {
    let delta = delta as i64;
    (delta != 0).then(|| Signal {
        unit,
        event: Event::Wheel {
            position: point(x, y),
            delta,
        },
    })
}

const LEAVE: Event = Event::Pointer {
    position: None,
    phase: Phase::Move,
};

#[derive(Default)]
pub(super) struct Pointer {
    pressed: bool,
    left: bool,
}

impl Pointer {
    pub fn moved(&mut self, point: Point, events: &mut Vec<Event>) {
        self.left = false;
        events.push(pointer(point, Phase::Move));
    }

    pub fn down(&mut self, point: Point, events: &mut Vec<Event>) {
        self.pressed = true;
        self.left = false;
        events.push(pointer(point, Phase::Down(Button::Primary)));
    }

    pub fn up(&mut self, point: Point, outside: bool, events: &mut Vec<Event>) {
        self.pressed = false;
        self.left = false;
        events.push(pointer(point, Phase::Up(Button::Primary)));
        if outside {
            self.leave(events);
        }
    }

    pub fn enter(&mut self, point: Point, events: &mut Vec<Event>) {
        if !self.pressed {
            self.moved(point, events);
        }
    }

    pub fn exit(&mut self, events: &mut Vec<Event>) {
        if !self.pressed {
            self.leave(events);
        }
    }

    fn leave(&mut self, events: &mut Vec<Event>) {
        if !self.left {
            self.left = true;
            events.push(LEAVE);
        }
    }
}

fn point(x: f64, y: f64) -> Point {
    Point {
        x: x.floor() as i64,
        y: y.floor() as i64,
    }
}

fn pointer(point: Point, phase: Phase) -> Event {
    Event::Pointer {
        position: Some(point),
        phase,
    }
}

#[path = "../../tests/macos/pointer.rs"]
mod checks;
