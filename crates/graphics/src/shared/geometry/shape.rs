use super::{bands::Bands, curve, prepare};
use crate::shared::{Clip, Curve, Fault, Path, Rect, Transform};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Evaluation {
    #[default]
    Linear,
    Quadratic,
}
#[derive(Debug)]
enum Segments {
    Linear(Vec<[f32; 4]>),
    Quadratic(Vec<Curve>),
}
#[derive(Debug)]
pub struct Geometry {
    id: u64,
    pub bounds: Rect,
    geometry: Segments,
    clip: Clip,
    bands: Bands,
}
impl Geometry {
    pub fn rectangle(clip: Clip) -> Result<Self, Fault> {
        let [left, top, right, bottom] = clip.edges();
        let edges = vec![
            [left, top, right, top],
            [right, top, right, bottom],
            [right, bottom, left, bottom],
            [left, bottom, left, top],
        ];
        Ok(Self {
            id: identifier()?,
            bounds: clip.bounds(),
            clip,
            bands: Bands::new(&edges, clip.bounds()),
            geometry: Segments::Linear(edges),
        })
    }

    pub fn new(path: &Path, transform: Transform, clip: Rect) -> Result<Self, Fault> {
        Self::clipped(path, transform, clip.into())
    }
    pub fn clipped(path: &Path, transform: Transform, clip: Clip) -> Result<Self, Fault> {
        let mut result = Self {
            id: identifier()?,
            bounds: clip.bounds(),
            clip,
            geometry: Segments::Linear(prepare::edges(path, transform)?),
            bands: Bands::default(),
        };
        result.crop(clip.bounds());
        result.bands = Bands::new(result.edges(), result.bounds);
        Ok(result)
    }
    pub fn quadratic(path: &Path, transform: Transform, clip: Clip) -> Result<Self, Fault> {
        let mut result = Self {
            id: identifier()?,
            bounds: clip.bounds(),
            clip,
            geometry: Segments::Quadratic(curve::prepare(path, transform)?),
            bands: Bands::default(),
        };
        result.crop(clip.bounds());
        result.bands = Bands::new(&result.envelope(), result.bounds);
        Ok(result)
    }
    pub fn evaluation(&self) -> Evaluation {
        match self.geometry {
            Segments::Linear(_) => Evaluation::Linear,
            Segments::Quadratic(_) => Evaluation::Quadratic,
        }
    }
    pub fn curves(&self) -> &[Curve] {
        match &self.geometry {
            Segments::Quadratic(curves) => curves,
            _ => &[],
        }
    }
    fn envelope(&self) -> std::borrow::Cow<'_, [[f32; 4]]> {
        match &self.geometry {
            Segments::Linear(edges) => std::borrow::Cow::Borrowed(edges),
            Segments::Quadratic(curves) => {
                std::borrow::Cow::Owned(curves.iter().map(Curve::envelope).collect())
            }
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn retained(&self) -> usize {
        std::mem::size_of::<Self>()
            + match &self.geometry {
                Segments::Linear(edges) => edges.capacity() * 16,
                Segments::Quadratic(curves) => curves.capacity() * std::mem::size_of::<Curve>(),
            }
            + self.bands.retained()
    }
    pub fn clip(&self) -> Clip {
        self.clip
    }
    pub fn edges(&self) -> &[[f32; 4]] {
        match &self.geometry {
            Segments::Linear(edges) => edges,
            _ => &[],
        }
    }
    pub fn bands(&self) -> &Bands {
        &self.bands
    }
    pub fn visits(&self) -> u64 {
        self.bands
            .visits()
            .saturating_mul(self.bounds.width as u64)
            .saturating_mul(16)
    }
    pub fn samples(&self, x: usize, y: usize) -> u8 {
        if !self.bounds.contains(x, y) {
            return 0;
        }
        let mut count = 0;
        for row in 0..4 {
            let vertical = y as f32 + (row as f32 + 0.5) / 4.0;
            let candidates = self.bands.candidates(vertical as usize);
            for column in 0..4 {
                let point = [x as f32 + (column as f32 + 0.5) / 4.0, vertical];
                count += u8::from(self.inside(point, candidates));
            }
        }
        count
    }
    fn inside(&self, point: [f32; 2], candidates: &[u32]) -> bool {
        if !self.clip.contains(point) {
            return false;
        }
        if let Segments::Quadratic(curves) = &self.geometry {
            return candidates
                .iter()
                .map(|&index| {
                    let curve = &curves[index as usize];
                    i32::from(curve.crossing(point[1]).is_some_and(|x| x > point[0]))
                        * curve.direction
                })
                .sum::<i32>()
                != 0;
        }
        let mut winding = 0;
        for &index in candidates {
            let [x, y, u, v] = self.edges()[index as usize];
            if (y <= point[1] && v > point[1]) || (v <= point[1] && y > point[1]) {
                let cross = x + (point[1] - y) / (v - y) * (u - x);
                let direction = i32::from(v > y) * 2 - 1;
                winding += i32::from(cross > point[0]) * direction;
            }
        }
        winding != 0
    }
    fn crop(&mut self, clip: Rect) {
        let mut low = [f32::INFINITY; 2];
        let mut high = [f32::NEG_INFINITY; 2];
        for edge in self.envelope().iter() {
            for axis in 0..2 {
                low[axis] = low[axis].min(edge[axis]).min(edge[axis + 2]);
                high[axis] = high[axis].max(edge[axis]).max(edge[axis + 2]);
            }
        }
        let x = (low[0].floor().max(0.0) as usize).max(clip.x);
        let y = (low[1].floor().max(0.0) as usize).max(clip.y);
        let right = (high[0].ceil().max(0.0) as usize).min(clip.x.saturating_add(clip.width));
        let bottom = (high[1].ceil().max(0.0) as usize).min(clip.y.saturating_add(clip.height));
        self.bounds = Rect {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        };
    }
}

fn identifier() -> Result<u64, Fault> {
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Fault("geometry identifiers exhausted".into()))
}
