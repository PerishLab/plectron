use super::{Focus, Image, Receipt, Signal, Unit, Visibility, Window};
use graphics::shared::{Backend as _, Cpu, Frame, Pacing, Pulse, State};

pub trait Display<E> {
    fn prepare(&mut self) -> Result<(), E> {
        Ok(())
    }
    fn create(&mut self, unit: Unit, options: &Window) -> Result<(), E>;
    fn activate(&mut self, unit: Unit) -> Result<(), E> {
        let _ = unit;
        Ok(())
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), E>;
    fn present(&mut self, unit: Unit, frame: Image) -> Result<(), E>;
    fn draw(&mut self, unit: Unit, frame: &Frame) -> Result<State, E> {
        let image = match Cpu.render(frame) {
            Ok(image) => image,
            Err(fault) => return Ok(State::Failed(fault)),
        };
        self.present(
            unit,
            Image {
                extent: super::Extent {
                    width: image.extent.width,
                    height: image.extent.height,
                },
                pixels: image.pixels,
            },
        )
        .map(|()| State::Complete)
    }
    fn completed(&mut self) -> Vec<Receipt<E>> {
        Vec::new()
    }
    fn visible(&mut self) -> Vec<Visibility> {
        Vec::new()
    }
    fn pulse(&mut self, unit: Unit) -> Result<Option<Pulse>, E> {
        let _ = unit;
        Ok(Some(Pulse::now()))
    }
    fn pace(&mut self, unit: Unit, pacing: Pacing) -> Result<(), E> {
        let _ = (unit, pacing);
        Ok(())
    }
    fn notifier(&self) -> shared::Notify {
        shared::Notify::thread(std::thread::current())
    }
    fn wait(&mut self, pacing: Pacing) -> Result<(), E> {
        match pacing {
            Pacing::Continuous => std::thread::park_timeout(std::time::Duration::from_millis(16)),
            Pacing::At(deadline) => std::thread::park_timeout(
                deadline.saturating_duration_since(std::time::Instant::now()),
            ),
            Pacing::Idle => std::thread::park(),
        }
        Ok(())
    }
    fn close(&mut self, unit: Unit) -> Result<(), E>;
}

pub trait Input<E> {
    fn poll(&mut self) -> Result<Vec<Signal>, E>;
    fn focus(&mut self, unit: Unit, context: Option<&Focus>) -> Result<(), E> {
        let _ = (unit, context);
        Ok(())
    }
}

pub trait Console<E> {
    fn log(&mut self, text: &str) -> Result<(), E>;
}

pub trait Backend<E>: Display<E> + Input<E> + Console<E> {}
impl<T: Display<E> + Input<E> + Console<E>, E> Backend<E> for T {}
