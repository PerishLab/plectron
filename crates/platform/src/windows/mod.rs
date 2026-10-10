mod class;
mod event;
mod window;
use crate::shared::{Console, Display, Event, Image, Input, Signal, Unit, Window as Options};
use graphics::shared::Pacing;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for Error {}
fn error(value: impl std::fmt::Display) -> Error {
    Error(value.to_string())
}
pub struct Desktop<E> {
    class: Rc<class::Class>,
    events: event::Events,
    windows: BTreeMap<Unit, window::Window>,
    convert: fn(Error) -> E,
}
impl<E> Desktop<E> {
    pub fn new(name: &str, convert: fn(Error) -> E) -> Result<Self, E> {
        if name != "cpu" {
            return Err(convert(Error(
                "Windows desktop currently requires the CPU renderer".into(),
            )));
        }
        Ok(Self {
            class: Rc::new(class::Class::new().map_err(convert)?),
            events: event::Events::new().map_err(convert)?,
            windows: BTreeMap::new(),
            convert,
        })
    }
    fn window(&self, unit: Unit) -> Result<&window::Window, Error> {
        self.windows
            .get(&unit)
            .ok_or_else(|| Error(format!("native window {} is absent", unit.id())))
    }
}
impl<E> Display<E> for Desktop<E> {
    fn create(&mut self, unit: Unit, options: &Options) -> Result<(), E> {
        if self.windows.contains_key(&unit) {
            return Err((self.convert)(Error(
                "native window identity already exists".into(),
            )));
        }
        let window = window::Window::new(self.class.clone(), options).map_err(self.convert)?;
        self.windows.insert(unit, window);
        Ok(())
    }
    fn activate(&mut self, unit: Unit) -> Result<(), E> {
        self.window(unit)
            .and_then(window::Window::show)
            .map_err(self.convert)
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.title(title))
            .map_err(self.convert)
    }
    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.present(frame))
            .map_err(self.convert)
    }
    fn notifier(&self) -> shared::Notify {
        self.events.notifier()
    }
    fn wait(&mut self, pacing: Pacing) -> Result<(), E> {
        self.class
            .check()
            .and_then(|()| self.events.wait(pacing))
            .map_err(self.convert)
    }
    fn close(&mut self, unit: Unit) -> Result<(), E> {
        self.class.check().map_err(self.convert)?;
        self.windows
            .remove(&unit)
            .ok_or_else(|| (self.convert)(Error("native window already closed".into())))?;
        Ok(())
    }
}
impl<E> Input<E> for Desktop<E> {
    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        self.class
            .check()
            .and_then(|()| self.events.poll())
            .map_err(self.convert)?;
        for window in self.windows.values() {
            window.result().map_err(self.convert)?;
        }
        Ok(self
            .windows
            .iter()
            .filter_map(|(unit, window)| {
                window.closing().then_some(Signal {
                    unit: *unit,
                    event: Event::Close,
                })
            })
            .collect())
    }
}
impl<E> Console<E> for Desktop<E> {
    fn log(&mut self, text: &str) -> Result<(), E> {
        println!("{text}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Extent;

    #[test]
    fn refusal() {
        assert!(Desktop::<Error>::new("metal", |fault| fault).is_err());
        let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
        let unit = Unit::new(1);
        let mut options = Options {
            title: "invalid\0title".into(),
            extent: Extent {
                width: 64,
                height: 32,
            },
        };
        assert!(desktop.create(unit, &options).is_err());
        options.title = "Plectron ownership probe".into();
        desktop.create(unit, &options).unwrap();
        assert!(desktop.create(unit, &options).is_err());
        desktop.close(unit).unwrap();
        assert!(desktop.title(unit, "closed").is_err());
        assert!(desktop.close(unit).is_err());
    }
}
