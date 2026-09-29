mod row;
mod scan;
use crate::shared::{Fault, Geometry, Rect};
use std::{
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mask {
    Solid,
    Samples(Range<usize>),
}
#[derive(Debug, Clone)]
pub struct Strip {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub mask: Mask,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Work {
    pub visits: u64,
    pub crossings: u64,
}
#[derive(Debug)]
pub struct Coverage {
    id: u64,
    bounds: Rect,
    strips: Vec<Strip>,
    samples: Vec<u8>,
    work: Work,
}
impl Coverage {
    pub fn new(geometry: &Geometry) -> Result<Self, Fault> {
        let mut output = Self {
            id: NEXT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .map_err(|_| Fault("coverage identifiers exhausted".into()))?,
            bounds: geometry.bounds,
            strips: Vec::new(),
            samples: Vec::new(),
            work: Work::default(),
        };
        if output.bounds.width == 0 || output.bounds.height == 0 {
            return Ok(output);
        }
        crate::shared::Extent {
            width: output.bounds.width,
            height: output.bounds.height,
        }
        .area()?;
        if geometry.bands().visits().saturating_mul(4) > 64_000_000 {
            return Err(Fault("scanline work budget exhausted".into()));
        }
        let mut scan = scan::Scan::default();
        for y in output.bounds.y..output.bounds.y + output.bounds.height {
            scan.row(geometry, y, &mut output)?;
        }
        Ok(output)
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn bounds(&self) -> Rect {
        self.bounds
    }
    pub fn strips(&self) -> &[Strip] {
        &self.strips
    }
    pub fn samples(&self) -> &[u8] {
        &self.samples
    }
    pub fn work(&self) -> Work {
        self.work
    }
    pub fn retained(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.strips.capacity() * std::mem::size_of::<Strip>()
            + self.samples.capacity()
    }
    fn emit(&mut self, y: usize, range: Range<usize>, count: u8) -> Result<(), Fault> {
        if count == 0 || range.is_empty() {
            return Ok(());
        }
        let solid = count == 16;
        let start = self.samples.len();
        if !solid {
            if start + range.len() > 4_194_304 {
                return Err(Fault("coverage sample budget exhausted".into()));
            }
            self.samples.extend(std::iter::repeat_n(count, range.len()));
        }
        if let Some(last) = self.strips.last_mut().filter(|last| {
            last.y == y
                && last.x + last.width == range.start
                && matches!(last.mask, Mask::Solid) == solid
        }) {
            last.width += range.len();
            if let Mask::Samples(samples) = &mut last.mask {
                samples.end = self.samples.len();
            }
            return Ok(());
        }
        if self.strips.len() >= 262_144 {
            return Err(Fault("coverage strip budget exhausted".into()));
        }
        self.strips.push(Strip {
            x: range.start,
            y,
            width: range.len(),
            mask: if solid {
                Mask::Solid
            } else {
                Mask::Samples(start..self.samples.len())
            },
        });
        Ok(())
    }
}
