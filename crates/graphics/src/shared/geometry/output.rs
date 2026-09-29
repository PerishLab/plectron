use crate::shared::{Clip, Extent, Fault, Scalar, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    logical: Extent,
    scale: Scalar,
    pixels: Extent,
}
impl Output {
    pub fn new(logical: Extent, scale: f64) -> Result<Self, Fault> {
        logical.area()?;
        if !scale.is_finite() || !(0.25..=8.0).contains(&scale) {
            return Err(Fault("device scale requires 0.25 through 8".into()));
        }
        let scale = Scalar::new(scale);
        let pixels = Extent {
            width: Scalar::new(logical.width as f64 * scale.value()).ceil(),
            height: Scalar::new(logical.height as f64 * scale.value()).ceil(),
        };
        pixels.area()?;
        Ok(Self {
            logical,
            scale,
            pixels,
        })
    }
    pub fn logical(self) -> Extent {
        self.logical
    }
    pub fn scale(self) -> f64 {
        self.scale.value()
    }
    pub fn pixels(self) -> Extent {
        self.pixels
    }
    pub fn transform(self, transform: Transform) -> Transform {
        Transform(
            transform
                .0
                .map(|value| Scalar::new(value.value() * self.scale())),
        )
    }
    pub fn clip(self, rect: crate::shared::Rect) -> Clip {
        Clip::from(rect).scale(self.scale())
    }
}
