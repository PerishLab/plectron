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
        for event in self.window.drain()? {
            match event {
                window::Event::Close => result = Ok(Some(Event::Close)),
                window::Event::Move { x, y } => signals.push(Signal {
                    unit,
                    event: Event::Pointer {
                        position: Some(point(x, y)),
                        phase: Phase::Move,
                    },
                }),
                window::Event::Down { x, y } => signals.push(Signal {
                    unit,
                    event: Event::Pointer {
                        position: Some(point(x, y)),
                        phase: Phase::Down(Button::Primary),
                    },
                }),
                window::Event::Up { x, y } => signals.push(Signal {
                    unit,
                    event: Event::Pointer {
                        position: Some(point(x, y)),
                        phase: Phase::Up(Button::Primary),
                    },
                }),
                window::Event::Wheel { x, y, delta } => signals.extend(wheel(unit, x, y, delta)),
                window::Event::Key {
                    code,
                    repeat,
                    flags,
                } => self.input.key(code, repeat, flags),
            }
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

fn point(x: f64, y: f64) -> Point {
    Point {
        x: x.max(0.0) as i64,
        y: y.max(0.0) as i64,
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
