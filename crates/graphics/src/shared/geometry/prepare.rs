use crate::shared::{Fault, Path, Segment, Transform};

pub(super) fn edges(path: &Path, transform: Transform) -> Result<Vec<[f32; 4]>, Fault> {
    let mut result = Builder { edges: Vec::new() };
    let mut start = [0.0; 2];
    let mut cursor = start;
    for segment in path.segments() {
        match segment {
            Segment::Move(point) => {
                cursor = transform.point(*point);
                start = cursor;
            }
            Segment::Line(point) => {
                let point = transform.point(*point);
                result.line(cursor, point)?;
                cursor = point;
            }
            Segment::Quad(control, end) => {
                let end = transform.point(*end);
                result.quad([cursor, transform.point(*control), end], 0)?;
                cursor = end;
            }
            Segment::Close => result.line(cursor, start)?,
        }
    }
    Ok(result.edges)
}
struct Builder {
    edges: Vec<[f32; 4]>,
}
impl Builder {
    fn line(&mut self, start: [f64; 2], end: [f64; 2]) -> Result<(), Fault> {
        if start == end {
            return Ok(());
        }
        if self.edges.len() >= 65_536
            || start
                .iter()
                .chain(&end)
                .any(|v| !v.is_finite() || v.abs() > 16_777_216.0)
        {
            return Err(Fault("prepared path budget exhausted".into()));
        }
        self.edges.push([
            start[0] as f32,
            start[1] as f32,
            end[0] as f32,
            end[1] as f32,
        ]);
        Ok(())
    }
    fn quad(&mut self, points: [[f64; 2]; 3], depth: usize) -> Result<(), Fault> {
        let [a, b, c] = points;
        let error = (b[0] - (a[0] + c[0]) / 2.0).hypot(b[1] - (a[1] + c[1]) / 2.0);
        if error <= 0.0625 {
            return self.line(a, c);
        }
        if depth == 16 {
            return Err(Fault("curve subdivision budget exhausted".into()));
        }
        let left = midpoint(a, b);
        let right = midpoint(b, c);
        let middle = midpoint(left, right);
        self.quad([a, left, middle], depth + 1)?;
        self.quad([middle, right, c], depth + 1)
    }
}
fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}
