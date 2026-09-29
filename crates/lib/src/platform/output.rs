use super::{Backend, Host, Pending, Platform};
use crate::contract::{Event, Fault, Signal, Unit};
use crate::platform::device::{self, Phase};
use crate::rendering::Renderer;
use webengine::State;

pub type Receipt = ::platform::shared::Receipt<Fault>;

impl<B: Backend, R: Renderer> Platform<B, R> {
    pub fn scroll(&mut self, unit: Unit, delta: i64) -> Result<(), Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .scroll(delta)?;
        self.submit(unit)
    }
    pub fn pan(&mut self, unit: Unit, target: (u64, u64), delta: i64) -> Result<(), Fault> {
        self.check(unit)?;
        self.pages
            .get_mut(&unit)
            .ok_or_else(|| Fault("unit has no web content".into()))?
            .pan(target.0, target.1, delta)?;
        self.submit(unit)
    }
    pub(super) fn admit(&mut self, signal: device::Signal) -> Result<(), Fault> {
        let visual = match signal.event {
            device::Event::Scroll { delta } => Some(self.scroll(signal.unit, delta)),
            device::Event::Wheel { position, delta } => {
                Some(self.wheel(signal.unit, (position.x, position.y), delta))
            }
            device::Event::Pointer { position, phase } => {
                Some(self.pointer(signal.unit, position, phase))
            }
            _ => None,
        };
        if let Some(result) = visual {
            if let Err(fault) = result {
                self.pending.push_back(Pending::Native(Signal {
                    unit: signal.unit,
                    event: Event::Error(fault.to_string()),
                }));
            }
            return Ok(());
        }
        let event = match signal.event {
            device::Event::Close => Event::Close,
            device::Event::Input(input) => Event::Input(input),
            device::Event::Composition(input) => Event::Composition(composition(input)),
            _ => unreachable!("visual device events returned above"),
        };
        self.pending.push_back(Pending::Native(Signal {
            unit: signal.unit,
            event,
        }));
        Ok(())
    }
    fn pointer(
        &mut self,
        unit: Unit,
        position: Option<device::Point>,
        phase: Phase,
    ) -> Result<(), Fault> {
        let point = position.map(|point| (point.x, point.y));
        match phase {
            Phase::Move => {
                self.pointers.insert(unit, point);
                self.hover(unit)
            }
            Phase::Down(device::Button::Primary) => {
                let target =
                    point.and_then(|(x, y)| self.pages.get(&unit).map(|page| page.hit(x, y)));
                self.presses.insert(
                    unit,
                    super::Press {
                        content: self.pages.contains_key(&unit),
                        target,
                    },
                );
                Ok(())
            }
            Phase::Up(device::Button::Primary) => {
                let Some(press) = self.presses.remove(&unit) else {
                    return Ok(());
                };
                if !press.content {
                    self.pending.push_back(Pending::Native(Signal {
                        unit,
                        event: Event::Click,
                    }));
                    return Ok(());
                }
                let Some(page) = self.pages.get(&unit) else {
                    return Ok(());
                };
                let Some((x, y)) = point else { return Ok(()) };
                let target = page.hit(x, y);
                if press.target.as_ref().and_then(webengine::Hit::target) == target.target() {
                    self.pending.push_back(Pending::Native(Signal {
                        unit,
                        event: Event::Target(target),
                    }));
                }
                Ok(())
            }
            Phase::Cancel => {
                self.presses.remove(&unit);
                Ok(())
            }
        }
    }
    fn hover(&mut self, unit: Unit) -> Result<(), Fault> {
        let point = self.pointers.get(&unit).copied().flatten();
        let Some(page) = self.pages.get_mut(&unit) else {
            return Ok(());
        };
        if page.state() != State::Ready || page.presented().is_none() {
            return Ok(());
        }
        let next = point.map(|(x, y)| page.hit(x, y));
        let previous = self.hovered.get(&unit).cloned().flatten();
        let same = previous.as_ref().and_then(webengine::Hit::target)
            == next.as_ref().and_then(webengine::Hit::target);
        if !same {
            page.transition(previous.as_ref(), next.as_ref())?;
        }
        self.hovered.insert(unit, next);
        Ok(())
    }
    pub fn wheel(&mut self, unit: Unit, point: (usize, usize), delta: i64) -> Result<(), Fault> {
        let Some(page) = self.pages.get_mut(&unit) else {
            return Ok(());
        };
        if page.wheel(point.0, point.1, delta)?.applied() {
            self.submit(unit)?;
        }
        Ok(())
    }
    pub fn stamp(&self, mut signal: Signal) -> Signal {
        if let Event::Pointer { x, y } = signal.event
            && let Some(page) = self.pages.get(&signal.unit)
        {
            signal.event = Event::Target(page.hit(x, y));
        }
        signal
    }

    pub(super) fn progress(&mut self) -> Result<(), Fault> {
        for receipt in self.backend.completed() {
            self.complete(receipt)?;
        }
        for receipt in self.backend.visible() {
            self.visible(receipt);
        }
        for unit in self.pages.keys().copied().collect::<Vec<_>>() {
            let pulse = self.backend.pulse(unit)?;
            let page = self.pages.get_mut(&unit).expect("owned page");
            if let Some(pulse) = pulse {
                page.pulse(pulse)?;
            }
            let events = page.poll()?;
            self.backend.pace(unit, page.pacing()?)?;
            self.events(unit, events);
            self.submit(unit)?;
        }
        Ok(())
    }

    pub(super) fn submit(&mut self, unit: Unit) -> Result<(), Fault> {
        let page = self.pages.get_mut(&unit).expect("owned page");
        if page.state() != State::Ready {
            return Ok(());
        }
        let Some(frame) = page.frame() else {
            return Ok(());
        };
        let context = page.focus(&frame).map(focus);
        self.backend.focus(unit, context.as_ref())?;
        let extent = self.units[&unit].extent;
        if frame.extent().width != extent.width || frame.extent().height != extent.height {
            return Err(Fault("content exceeded assigned extent".into()));
        }
        self.query
            .admit(crate::ipc::query::Target::Frame {
                unit: unit.id(),
                frame: frame.id(),
                scene: frame.scene().id(),
            })
            .map_err(Fault)?;
        let result = match self.backend.draw(unit, &frame) {
            Ok(graphics::shared::State::Pending) => return Ok(()),
            Ok(graphics::shared::State::Complete) => Ok(()),
            Ok(graphics::shared::State::Failed(fault)) => Err(Fault(fault.0)),
            Err(fault) => Err(fault),
        };
        self.complete(Receipt {
            unit,
            frame: frame.id(),
            produced: Some(std::time::Instant::now()),
            visible: None,
            drops: 0,
            result,
        })
    }

    fn complete(&mut self, receipt: Receipt) -> Result<(), Fault> {
        let Receipt {
            unit,
            frame,
            produced,
            visible,
            drops,
            result: completion,
        } = receipt;
        let Some(page) = self.pages.get_mut(&unit) else {
            return Ok(());
        };
        let event = completion
            .as_ref()
            .err()
            .map(|fault| Event::Error(fault.to_string()));
        let result = completion
            .as_ref()
            .map(|()| String::new())
            .map_err(ToString::to_string);
        let first = page.complete(frame, completion)?;
        if let Some(id) = self.query.frame(unit.id(), frame) {
            self.query.finish(
                id,
                crate::ipc::query::State::Complete { result },
                crate::ipc::query::Production {
                    produced,
                    visible,
                    drops,
                },
            );
        }
        if let Some(event) = event.or(first.then_some(Event::Ready)) {
            self.pending
                .push_back(Pending::Native(Signal { unit, event }));
        }
        self.hover(unit)
    }

    fn visible(&mut self, receipt: ::platform::shared::Visibility) {
        let Some(id) = self.query.frame(receipt.unit.id(), receipt.frame) else {
            return;
        };
        self.query.visible(id, receipt.visible, receipt.drops);
    }
}

fn composition(input: device::Composition) -> webengine::ime::Input {
    match input {
        device::Composition::Mark {
            id,
            text,
            selection,
        } => webengine::ime::Input::Mark {
            id: webengine::ime::Id(id.0),
            text,
            selection: domengine::Selection {
                anchor: selection.anchor,
                focus: selection.focus,
            },
        },
        device::Composition::Commit { id, text } => webengine::ime::Input::Commit {
            id: webengine::ime::Id(id.0),
            text,
        },
        device::Composition::Cancel { id } => webengine::ime::Input::Cancel {
            id: webengine::ime::Id(id.0),
        },
    }
}

fn focus(context: webengine::Focus) -> ::platform::shared::Focus {
    ::platform::shared::Focus {
        target: context.target,
        text: context.text,
        selection: ::platform::shared::Selection {
            anchor: context.selection.anchor,
            focus: context.selection.focus,
        },
        caret: ::platform::shared::Caret {
            x: context.caret.x,
            y: context.caret.y,
            width: context.caret.width,
            height: context.caret.height,
        },
    }
}
