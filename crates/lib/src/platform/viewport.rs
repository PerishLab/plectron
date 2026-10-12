use super::{Backend, Host, Platform};
use crate::{
    contract::{Fault, Unit},
    rendering::Renderer,
};

impl<B: Backend, R: Renderer> Platform<B, R> {
    pub(super) fn viewport(
        &mut self,
        unit: Unit,
        size: (usize, usize),
        density: (u32, u32),
    ) -> Result<(), Fault> {
        self.check(unit)?;
        if density.1 == 0 {
            return Err(Fault("viewport density denominator is zero".into()));
        }
        let extent = domengine::Extent {
            width: size.0,
            height: size.1,
        };
        graphics::shared::Output::new(extent, f64::from(density.0) / f64::from(density.1))
            .map_err(|fault| Fault(fault.to_string()))?;
        if let Some(page) = self.pages.get_mut(&unit) {
            page.resize(extent)?;
        }
        self.units.get_mut(&unit).expect("checked unit").extent = crate::contract::Extent {
            width: size.0,
            height: size.1,
        };
        self.presses.remove(&unit);
        self.pointers.remove(&unit);
        self.hovered.remove(&unit);
        Ok(())
    }
}
