use super::{Fault, Scalar};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

pub type Point = [Scalar; 2];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Segment {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Close,
}
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Path {
    data: Arc<Data>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
struct Data {
    segments: Box<[Segment]>,
    #[cfg_attr(feature = "serde", serde(skip))]
    bounds: Option<[Scalar; 4]>,
    #[cfg_attr(feature = "serde", serde(skip))]
    hash: u64,
}

impl Path {
    pub fn new(segments: Vec<Segment>) -> Result<Self, Fault> {
        if segments.len() > 65_536 {
            return Err(Fault("path segment budget exhausted".into()));
        }
        let mut open = false;
        let mut bounds: Option<[Scalar; 4]> = None;
        for segment in &segments {
            let points: &[_] = match segment {
                Segment::Move(point) if !open => {
                    open = true;
                    std::slice::from_ref(point)
                }
                Segment::Line(point) if open => std::slice::from_ref(point),
                Segment::Quad(control, point) if open => &[*control, *point],
                Segment::Close if open => {
                    open = false;
                    &[]
                }
                _ => return Err(Fault("path requires explicitly closed contours".into())),
            };
            for point in points {
                let [x, y] = *point;
                bounds = Some(bounds.map_or([x, y, x, y], |b| {
                    [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
                }));
            }
            if points
                .iter()
                .flatten()
                .any(|value| value.value().abs() > 1_048_576.0)
            {
                return Err(Fault("path coordinate budget exhausted".into()));
            }
        }
        if open {
            return Err(Fault("unclosed path contour".into()));
        }
        let mut hash = DefaultHasher::new();
        segments.hash(&mut hash);
        Ok(Self {
            data: Arc::new(Data {
                segments: segments.into(),
                bounds,
                hash: hash.finish(),
            }),
        })
    }
    pub fn segments(&self) -> &[Segment] {
        &self.data.segments
    }
    pub fn bounds(&self) -> Option<[Scalar; 4]> {
        self.data.bounds
    }
    pub fn retained(&self) -> usize {
        std::mem::size_of::<Data>() + std::mem::size_of_val(self.segments())
    }
}
impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash.hash(state);
    }
}
impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            || (self.data.hash == other.data.hash && self.data.segments == other.data.segments)
    }
}
impl Eq for Path {}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Path {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<Segment>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Transform(pub [Scalar; 6]);
impl Default for Transform {
    fn default() -> Self {
        Self([1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()])
    }
}
impl Transform {
    pub fn point(self, point: Point) -> [f64; 2] {
        let [a, b, c, d, x, y] = self.0.map(Scalar::value);
        [
            a * point[0].value() + c * point[1].value() + x,
            b * point[0].value() + d * point[1].value() + y,
        ]
    }
    pub fn translate(mut self, offset: [Scalar; 2]) -> Self {
        self.0[4] = self.0[4] + offset[0];
        self.0[5] = self.0[5] + offset[1];
        self
    }
}
