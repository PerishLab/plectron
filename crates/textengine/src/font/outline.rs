use super::{
    Problem, Source,
    data::{Cursor, Data},
};
use crate::{Path, Point, Scalar, Segment};

pub(super) struct Outline {
    pub points: Vec<(Point, bool)>,
    pub ends: Vec<usize>,
}
impl Source {
    pub(super) fn contours(
        &self,
        glyph: u16,
        stack: &mut Vec<u16>,
        budget: &mut usize,
    ) -> Result<Outline, Problem> {
        if stack.len() >= 16 || stack.contains(&glyph) {
            return Err(Problem::Malformed);
        }
        *budget = budget.checked_sub(1).ok_or(Problem::Budget)?;
        let data = self.glyph(glyph)?;
        let mut outline = Outline {
            points: Vec::new(),
            ends: Vec::new(),
        };
        if data.0.is_empty() {
            return Ok(outline);
        }
        data.slice(0, 10)?;
        let count = data.signed(0)?;
        stack.push(glyph);
        if count >= 0 {
            outline.simple(data, count as usize, budget)?;
        } else {
            self.compound(data, &mut outline, (stack, budget))?;
        }
        stack.pop();
        Ok(outline)
    }
}
impl Outline {
    fn simple(&mut self, data: Data<'_>, count: usize, budget: &mut usize) -> Result<(), Problem> {
        if count == 0 {
            return Ok(());
        }
        let mut cursor = Cursor { data, offset: 10 };
        for _ in 0..count {
            let end = usize::from(cursor.word()?);
            if self.ends.last().is_some_and(|previous| *previous >= end) {
                return Err(Problem::Malformed);
            }
            self.ends.push(end);
        }
        let count = self.ends[count - 1] + 1;
        *budget = budget.checked_sub(count).ok_or(Problem::Budget)?;
        let instructions = usize::from(cursor.word()?);
        cursor.skip(instructions)?;
        let mut flags = Vec::with_capacity(count);
        while flags.len() < count {
            let flag = cursor.byte()?;
            if flag & 128 != 0 {
                return Err(Problem::Malformed);
            }
            let repeat = if flag & 8 != 0 {
                usize::from(cursor.byte()?)
            } else {
                0
            };
            if flags.len() + repeat + 1 > count {
                return Err(Problem::Malformed);
            }
            flags.extend(std::iter::repeat_n(flag, repeat + 1));
        }
        let x = coordinates(&mut cursor, &flags, 2)?;
        let y = coordinates(&mut cursor, &flags, 4)?;
        self.points = x
            .into_iter()
            .zip(y)
            .zip(flags)
            .map(|((x, y), flag)| ([x.into(), y.into()], flag & 1 != 0))
            .collect();
        Ok(())
    }
    pub fn path(self) -> Result<Path, Problem> {
        let mut segments = Vec::new();
        let mut start = 0;
        for end in self.ends {
            contour(&self.points[start..=end], &mut segments);
            start = end + 1;
        }
        Path::new(segments).map_err(|_| Problem::Budget)
    }
}
fn coordinates(cursor: &mut Cursor<'_>, flags: &[u8], short: u8) -> Result<Vec<i64>, Problem> {
    let mut output = Vec::with_capacity(flags.len());
    let mut value = 0;
    for &flag in flags {
        let delta = if flag & short != 0 {
            i64::from(cursor.byte()?) * if flag & (short << 3) != 0 { 1 } else { -1 }
        } else if flag & (short << 3) != 0 {
            0
        } else {
            i64::from(cursor.signed()?)
        };
        value += delta;
        if !(-32768..=32767).contains(&value) {
            return Err(Problem::Malformed);
        }
        output.push(value);
    }
    Ok(output)
}
fn contour(points: &[(Point, bool)], output: &mut Vec<Segment>) {
    let first = points[0];
    let last = points[points.len() - 1];
    let origin = if first.1 {
        first.0
    } else if last.1 {
        last.0
    } else {
        midpoint(first.0, last.0)
    };
    output.push(Segment::Move(origin));
    let mut control = None;
    for &(point, on) in points.iter().skip(usize::from(first.1)) {
        if on {
            output.push(control.take().map_or(Segment::Line(point), |previous| {
                Segment::Quad(previous, point)
            }));
        } else {
            if let Some(previous) = control {
                output.push(Segment::Quad(previous, midpoint(previous, point)));
            }
            control = Some(point);
        }
    }
    if let Some(control) = control {
        output.push(Segment::Quad(control, origin));
    }
    output.push(Segment::Close);
}
fn midpoint(a: Point, b: Point) -> Point {
    [
        Scalar::new((a[0].value() + b[0].value()) / 2.0),
        Scalar::new((a[1].value() + b[1].value()) / 2.0),
    ]
}
