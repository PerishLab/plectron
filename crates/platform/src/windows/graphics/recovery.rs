use super::super::{Desktop, Error};
use super::Recovery;
use crate::shared::Receipt;

impl<E> Desktop<E> {
    pub(in crate::windows) fn recover(&mut self, recovery: Recovery) -> Receipt<Error> {
        let unit = recovery.unit;
        let frame = recovery.frame;
        let image = recovery.image;
        let fault = recovery.fault;
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
}
