use super::{Face, Font, Source};
use crate::{Fault, Path, Scalar, Segment, Transform};
impl Font {
    pub fn source(&self) -> Option<&Source> {
        (matches!(self.face, Face::Owned(_))).then(|| self.vector())
    }
    pub(super) fn vector(&self) -> &Source {
        match &self.face {
            Face::Owned(lease) => lease.source(),
            Face::Toy | Face::Missing => unreachable!("toy face has no vector source"),
        }
    }

    pub fn advance(&self, glyph: u16) -> Result<Scalar, Fault> {
        self.validate()?;
        if !matches!(self.face, Face::Owned(_)) {
            return Ok((self.glyph(glyph)?.advance as i64).into());
        }
        Ok(Scalar::new(
            f64::from(self.vector().advance(glyph)?) * self.size.value()
                / f64::from(self.vector().units()),
        ))
    }
    pub fn ascent(&self) -> Scalar {
        if !matches!(self.face, Face::Owned(_)) {
            return (self.scale(7) as i64).into();
        }
        Scalar::new(
            f64::from(self.vector().ascent()) * self.size.value()
                / f64::from(self.vector().units()),
        )
    }
    pub fn height(&self) -> Scalar {
        if !matches!(self.face, Face::Owned(_)) {
            return self.size;
        }
        Scalar::new(
            f64::from(i32::from(self.vector().ascent()) - i32::from(self.vector().descent()))
                * self.size.value()
                / f64::from(self.vector().units()),
        )
    }
    pub fn outline(&self, glyph: u16) -> Result<Path, Fault> {
        if matches!(self.face, Face::Owned(_)) {
            return Ok(self.vector().outline(glyph)?);
        }
        let bitmap = self.glyph(glyph)?;
        let mut segments = Vec::new();
        for (y, row) in bitmap.rows.iter().enumerate() {
            for x in 0..bitmap.width {
                if row & (1 << x) != 0 {
                    square(&mut segments, x as i64, y as i64);
                }
            }
        }
        Ok(Path::new(segments)?)
    }
    pub fn transform(&self) -> Transform {
        let scale = self.size.value()
            / if !matches!(self.face, Face::Owned(_)) {
                8.0
            } else {
                f64::from(self.vector().units())
            };
        let vertical = if !matches!(self.face, Face::Owned(_)) {
            scale
        } else {
            -scale
        };
        Transform([
            Scalar::new(scale),
            0.into(),
            0.into(),
            Scalar::new(vertical),
            0.into(),
            if !matches!(self.face, Face::Owned(_)) {
                0.into()
            } else {
                self.ascent()
            },
        ])
    }
}
fn square(output: &mut Vec<Segment>, x: i64, y: i64) {
    output.extend([
        Segment::Move([x.into(), y.into()]),
        Segment::Line([(x + 1).into(), y.into()]),
        Segment::Line([(x + 1).into(), (y + 1).into()]),
        Segment::Line([x.into(), (y + 1).into()]),
        Segment::Close,
    ]);
}
