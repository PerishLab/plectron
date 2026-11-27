use super::super::{Desktop, Error};
use super::Recovery;
use crate::shared::{Receipt, Unit};

impl<E> Desktop<E> {
    pub(in crate::windows) fn recover(&mut self, recovery: Recovery) -> Receipt<Error> {
        let Recovery {
            unit,
            frame,
            image,
            fault,
        } = recovery;
        let Some(fault) = fault else {
            return self.hedge(unit, frame, image);
        };
        let result = self.graphics.close(unit).and_then(|()| {
            let window = self.window(unit)?;
            window.fallback(image, &self.timing, frame)
        });
        let result = result.map_err(|recovery| {
            Error(format!(
                "D3D12 rendering failed ({fault}); CPU recovery failed ({recovery})"
            ))
        });
        let produced = result.as_ref().ok().map(|()| std::time::Instant::now());
        if result.is_ok() {
            eprintln!(
                "[renderer:cpu] recovered frame={} after D3D12 failure: {}",
                frame, fault
            );
        }
        Receipt {
            unit,
            frame,
            produced,
            visible: None,
            drops: 0,
            result,
        }
    }

    fn hedge(&mut self, unit: Unit, frame: u64, image: graphics::shared::Image) -> Receipt<Error> {
        let result = self.window(unit).and_then(|window| {
            let outcome = window.fallback(image, &self.timing, frame);
            window.accelerate(true);
            outcome
        });
        let produced = result.as_ref().ok().map(|()| std::time::Instant::now());
        if let Some(at) = produced
            && self.timing.native()
        {
            self.graphics.watch(unit, frame, at);
        }
        Receipt {
            unit,
            frame,
            produced,
            visible: None,
            drops: 0,
            result,
        }
    }
}
