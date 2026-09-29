use crate::shared::{Fault, Path, Segment, Transform};

#[derive(Debug)]
pub struct Curve {
    pub points: [[f32; 2]; 3],
    pub direction: i32,
    pub quadratic: bool,
}
impl Curve {
    pub fn crossing(&self, y: f32) -> Option<f32> {
        let [p, q, r] = self.points;
        if y < p[1] || y >= r[1] {
            return None;
        }
        let dy = y - p[1];
        if !self.quadratic {
            return Some(p[0] + dy / (r[1] - p[1]) * (r[0] - p[0]));
        }
        let b = q[1] - p[1];
        let a = (r[1] - q[1]) - b;
        let divisor = b + (b * b + a * dy).max(0.0).sqrt();
        let t = if dy == 0.0 {
            0.0
        } else {
            (dy / divisor).clamp(0.0, 1.0)
        };
        let left = p[0] + t * (q[0] - p[0]);
        let right = q[0] + t * (r[0] - q[0]);
        Some(left + t * (right - left))
    }
    pub(super) fn envelope(&self) -> [f32; 4] {
        let [p, q, r] = self.points;
        [
            p[0].min(q[0]).min(r[0]),
            p[1],
            p[0].max(q[0]).max(r[0]),
            r[1],
        ]
    }
}
pub(super) fn prepare(path: &Path, transform: Transform) -> Result<Vec<Curve>, Fault> {
    let mut builder = Builder { curves: Vec::new() };
    let mut start = [0.0; 2];
    let mut cursor = start;
    for segment in path.segments() {
        match segment {
            Segment::Move(point) => {
                cursor = transform.point(*point);
                start = cursor;
            }
            Segment::Line(point) => {
                let end = transform.point(*point);
                builder.push([cursor, cursor, end], false)?;
                cursor = end;
            }
            Segment::Quad(control, end) => {
                let end = transform.point(*end);
                builder.quad([cursor, transform.point(*control), end])?;
                cursor = end;
            }
            Segment::Close => builder.push([cursor, cursor, start], false)?,
        }
    }
    Ok(builder.curves)
}
struct Builder {
    curves: Vec<Curve>,
}
impl Builder {
    fn quad(&mut self, points: [[f64; 2]; 3]) -> Result<(), Fault> {
        validate(points)?;
        let [p, q, r] = points;
        let divisor = (q[1] - p[1]) + (q[1] - r[1]);
        let t = (q[1] - p[1]) / divisor;
        if t > 0.0 && t < 1.0 {
            let left = lerp(p, q, t);
            let right = lerp(q, r, t);
            let middle = lerp(left, right, t);
            self.push([p, left, middle], true)?;
            self.push([middle, right, r], true)
        } else {
            self.push(points, true)
        }
    }
    fn push(&mut self, points: [[f64; 2]; 3], quadratic: bool) -> Result<(), Fault> {
        validate(points)?;
        let mut points = points.map(|point| point.map(|v| v as f32));
        let direction = if points[0][1] > points[2][1] {
            points.reverse();
            -1
        } else {
            1
        };
        if points[0][1] == points[2][1] {
            return Ok(());
        }
        if self.curves.len() >= 32_768 {
            return Err(Fault("prepared curve budget exhausted".into()));
        }
        points[1][1] = points[1][1].clamp(points[0][1], points[2][1]);
        self.curves.push(Curve {
            points,
            direction,
            quadratic,
        });
        Ok(())
    }
}
fn lerp(p: [f64; 2], q: [f64; 2], t: f64) -> [f64; 2] {
    [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]
}
fn validate(points: [[f64; 2]; 3]) -> Result<(), Fault> {
    if points
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 16_777_216.0)
    {
        return Err(Fault("prepared curve coordinate budget exhausted".into()));
    }
    Ok(())
}
