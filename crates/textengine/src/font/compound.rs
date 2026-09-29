use super::{
    Problem, Source,
    data::{Cursor, Data},
    outline::Outline,
};
use crate::{Point, Scalar, Transform};

impl Source {
    pub(super) fn compound(
        &self,
        data: Data<'_>,
        output: &mut Outline,
        context: (&mut Vec<u16>, &mut usize),
    ) -> Result<(), Problem> {
        let mut cursor = Cursor { data, offset: 10 };
        loop {
            let flags = cursor.word()?;
            let glyph = cursor.word()?;
            if flags & 0xe010 != 0 || flags & 0x1800 == 0x1800 {
                return Err(Problem::Malformed);
            }
            let args = arguments(&mut cursor, flags)?;
            let mut transform = matrix(&mut cursor, flags)?;
            let child = self.contours(glyph, context.0, context.1)?;
            let mut points: Vec<_> = child
                .points
                .iter()
                .map(|(point, on)| (transform.point(*point).map(Scalar::new), *on))
                .collect();
            let offset = if flags & 2 != 0 {
                if flags & 0x800 != 0 {
                    transform
                        .point(args.map(|v| Scalar::new(v as f64)))
                        .map(Scalar::new)
                } else {
                    args.map(|v| Scalar::new(v as f64))
                }
            } else {
                let parent = point(&output.points, args[0] as usize)?;
                let child = point(&points, args[1] as usize)?;
                [parent[0] - child[0], parent[1] - child[1]]
            };
            transform = Transform::default().translate(offset);
            for (point, _) in &mut points {
                *point = transform.point(*point).map(Scalar::new);
            }
            let base = output.points.len();
            output.ends.extend(child.ends.iter().map(|end| base + end));
            output.points.extend(points);
            if flags & 32 == 0 {
                if flags & 256 != 0 {
                    let size = usize::from(cursor.word()?);
                    cursor.skip(size)?;
                }
                return Ok(());
            }
        }
    }
}
fn arguments(cursor: &mut Cursor<'_>, flags: u16) -> Result<[i32; 2], Problem> {
    let mut args = [0; 2];
    for arg in &mut args {
        *arg = match (flags & 1 != 0, flags & 2 != 0) {
            (true, true) => i32::from(cursor.signed()?),
            (true, false) => i32::from(cursor.word()?),
            (false, true) => i32::from(cursor.byte()? as i8),
            (false, false) => i32::from(cursor.byte()?),
        };
    }
    Ok(args)
}
fn matrix(cursor: &mut Cursor<'_>, flags: u16) -> Result<Transform, Problem> {
    if (flags & (8 | 64 | 128)).count_ones() > 1 {
        return Err(Problem::Malformed);
    }
    let mut values = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    if flags & 8 != 0 {
        values[0] = cursor.factor()?;
        values[3] = values[0];
    }
    if flags & 64 != 0 {
        values[0] = cursor.factor()?;
        values[3] = cursor.factor()?;
    }
    if flags & 128 != 0 {
        for value in &mut values[..4] {
            *value = cursor.factor()?;
        }
    }
    Ok(Transform(values.map(Scalar::new)))
}

fn point(points: &[(Point, bool)], index: usize) -> Result<Point, Problem> {
    if let Some(point) = points.get(index) {
        return Ok(point.0);
    }
    Err(if index < points.len() + 4 {
        Problem::Unsupported
    } else {
        Problem::Malformed
    })
}
