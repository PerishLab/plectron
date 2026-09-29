use crate::shared::{Rect, Scalar};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Clip([Scalar; 4]);
impl From<Rect> for Clip {
    fn from(rect: Rect) -> Self {
        Self(
            [
                rect.x as f64,
                rect.y as f64,
                rect.x.saturating_add(rect.width) as f64,
                rect.y.saturating_add(rect.height) as f64,
            ]
            .map(Scalar::new),
        )
    }
}
impl Clip {
    pub(crate) fn rectangle(edges: [f32; 4]) -> Self {
        Self(edges.map(|value| Scalar::new(f64::from(value))))
    }
    pub(crate) fn intersect(self, other: Self) -> Self {
        Self([
            self.0[0].max(other.0[0]),
            self.0[1].max(other.0[1]),
            self.0[2].min(other.0[2]),
            self.0[3].min(other.0[3]),
        ])
    }

    pub fn scale(self, scale: f64) -> Self {
        Self(self.0.map(|value| Scalar::new(value.value() * scale)))
    }
    pub fn edges(self) -> [f32; 4] {
        self.0.map(|value| value.value() as f32)
    }
    pub fn contains(self, point: [f32; 2]) -> bool {
        let edges = self.edges();
        (0..2).all(|axis| point[axis] >= edges[axis] && point[axis] < edges[axis + 2])
    }
    pub fn integral(self) -> bool {
        self.0.iter().all(|value| value.value().fract() == 0.0)
    }
    pub fn bounds(self) -> Rect {
        let [left, top, right, bottom] = self.0;
        let x = left.floor().max(0) as usize;
        let y = top.floor().max(0) as usize;
        Rect {
            x,
            y,
            width: right.ceil().saturating_sub(x),
            height: bottom.ceil().saturating_sub(y),
        }
    }
}
