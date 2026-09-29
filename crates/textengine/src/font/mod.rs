use crate::{Fault, Scalar};
use serde::{Deserialize, Serialize};
mod compound;
pub(crate) mod data;
mod mapping;
mod outline;
mod source;
pub use source::Source;
mod lease;
mod vector;
pub use lease::Lease;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Malformed,
    Unsupported,
    Missing,
    Budget,
}
impl From<Problem> for Fault {
    fn from(problem: Problem) -> Self {
        Self(format!("font resource: {problem:?}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Face {
    Toy,
    Missing,
    Owned(Lease),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weight {
    Normal,
    Bold,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Font {
    pub face: Face,
    pub size: Scalar,
    pub weight: Weight,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            face: Face::Toy,
            size: 16.into(),
            weight: Weight::Normal,
        }
    }
}

pub struct Bitmap {
    pub rows: [u16; 8],
    pub width: usize,
    pub advance: usize,
}

impl Font {
    pub fn resource(&self) -> Option<shared::cache::Resource> {
        match &self.face {
            Face::Owned(lease) => Some(lease.resource()),
            _ => None,
        }
    }
    pub fn retained(&self) -> usize {
        match &self.face {
            Face::Owned(lease) => lease.source().bytes().len(),
            _ => 0,
        }
    }
    pub fn validate(&self) -> Result<(), Fault> {
        if self.size < Scalar::ZERO || self.size > 4096.into() {
            return Err(Fault("font size budget exhausted".into()));
        }
        Ok(())
    }
    pub fn scale(&self, units: usize) -> usize {
        (units as f64 * self.size.value() / 8.0).floor() as usize
    }
    pub fn index(&self, character: char) -> u16 {
        if self.face == Face::Missing {
            return if character.is_whitespace() { 32 } else { 63 };
        }
        if matches!(self.face, Face::Owned(_)) {
            return self
                .vector()
                .index(character)
                .expect("validated font cmap")
                .unwrap_or(0);
        }
        if character.is_ascii() {
            character as u16
        } else {
            b'?' as u16
        }
    }
    pub fn glyph(&self, id: u16) -> Result<Bitmap, Fault> {
        self.validate()?;
        if matches!(self.face, Face::Owned(_)) {
            return Err(Fault("vector font has no bitmap resource".into()));
        }
        let mut bitmap = bitmap(id)?;
        if self.weight == Weight::Bold {
            bitmap.rows = bitmap.rows.map(|row| row | (row << 1));
        }
        bitmap.advance = self.scale(bitmap.advance);
        Ok(bitmap)
    }
}

fn bitmap(id: u16) -> Result<Bitmap, Fault> {
    if id > 128 {
        return Err(Fault("unknown glyph resource".into()));
    }
    let width = if id == 128 { 12 } else { 8 };
    let rows = if id <= 32 {
        [0; 8]
    } else {
        [0, 126, 66, 66, 66, 66, 126, 0]
    };
    Ok(Bitmap {
        rows,
        width,
        advance: width,
    })
}
