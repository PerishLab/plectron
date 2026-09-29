use crate::shared::{Clip, Coverage, Fault, Path, Rect, Segment, Transform};
use std::sync::Arc;

pub(super) const LIMIT: f64 = 16_777_216.0;

#[derive(Debug, Clone)]
pub struct Placement {
    pub coverage: Arc<Coverage>,
    pub offset: [usize; 2],
}
impl Placement {
    pub fn bounds(&self) -> Rect {
        let mut bounds = self.coverage.bounds();
        bounds.x += self.offset[0];
        bounds.y += self.offset[1];
        bounds
    }
}

pub(super) fn normalize(
    path: &Path,
    transform: Transform,
    clip: Clip,
) -> Result<(Transform, Clip, [usize; 2]), Fault> {
    let hull = if transform.0[1] == 0 && transform.0[2] == 0 {
        bounds(path, transform)?
    } else {
        envelope(path, transform)?
    };
    let edges = clip.edges().map(f64::from);
    let low = [edges[0].max(hull[0].floor()), edges[1].max(hull[1].floor())];
    let high = [edges[2].min(hull[2].ceil()), edges[3].min(hull[3].ceil())];
    if (0..2).any(|axis| high[axis] <= low[axis]) {
        return Ok((transform, Clip::rectangle([0.0; 4]), [0; 2]));
    }
    let offset = low.map(|value| value.floor().max(0.0) as usize);
    let shift = offset.map(|value| -(value as f64));
    let clip = Clip::rectangle([
        (low[0] + shift[0]) as f32,
        (low[1] + shift[1]) as f32,
        (high[0] + shift[0]) as f32,
        (high[1] + shift[1]) as f32,
    ]);
    Ok((
        transform.translate(shift.map(crate::shared::Scalar::new)),
        clip,
        offset,
    ))
}

pub(super) fn envelope(path: &Path, transform: Transform) -> Result<[f64; 4], Fault> {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for segment in path.segments() {
        let points: &[_] = match segment {
            Segment::Move(point) | Segment::Line(point) => std::slice::from_ref(point),
            Segment::Quad(control, point) => &[*control, *point],
            Segment::Close => &[],
        };
        for point in points {
            let point = transform.point(*point);
            if point.iter().any(|v| !v.is_finite() || v.abs() > LIMIT) {
                return Err(Fault("transformed path coordinate budget exhausted".into()));
            }
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        }
    }
    Ok(bounds)
}

pub(super) fn bounds(path: &Path, transform: Transform) -> Result<[f64; 4], Fault> {
    let Some([left, top, right, bottom]) = path.bounds() else {
        return Ok([
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ]);
    };
    let corners = [[left, top], [right, top], [right, bottom], [left, bottom]]
        .map(|point| transform.point(point));
    if corners
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > LIMIT)
    {
        return envelope(path, transform);
    }
    Ok(corners.into_iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        },
    ))
}
