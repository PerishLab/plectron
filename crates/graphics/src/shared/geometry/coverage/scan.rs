use super::{Coverage, row::Row};
use crate::shared::{Evaluation, Fault, Geometry};
use std::ops::Range;

#[derive(Default)]
pub(super) struct Scan {
    events: Vec<(f32, i32)>,
    row: Row,
}
impl Scan {
    pub fn row(
        &mut self,
        geometry: &Geometry,
        y: usize,
        output: &mut Coverage,
    ) -> Result<(), Fault> {
        let clip = geometry.clip().edges();
        for sample in 0..4 {
            let ordinate = y as f32 + (sample as f32 + 0.5) / 4.0;
            if ordinate < clip[1] || ordinate >= clip[3] {
                continue;
            }
            self.crossings(geometry, ordinate, &mut output.work);
            self.intervals(geometry);
        }
        self.row.finish(y, output)
    }
    fn crossings(&mut self, geometry: &Geometry, y: f32, work: &mut super::Work) {
        self.events.clear();
        let candidates = geometry.bands().candidates(y as usize);
        work.visits += candidates.len() as u64;
        for &index in candidates {
            if let Some(event) = crossing(geometry, index as usize, y) {
                self.events.push(event);
            }
        }
        work.crossings += self.events.len() as u64;
        self.events.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    }
    fn intervals(&mut self, geometry: &Geometry) {
        let clip = geometry.clip().edges();
        let bounds = geometry.bounds;
        let samples = bounds.x * 4..(bounds.x + bounds.width) * 4;
        let mut winding: i32 = self.events.iter().map(|event| event.1).sum();
        let mut previous = clip[0];
        for &(cross, direction) in &self.events {
            let end = cross.clamp(clip[0], clip[2]);
            if winding != 0 {
                self.row
                    .interval(lower(samples.clone(), previous)..lower(samples.clone(), end));
            }
            winding -= direction;
            previous = end;
        }
        if winding != 0 {
            self.row
                .interval(lower(samples.clone(), previous)..lower(samples, clip[2]));
        }
    }
}
fn crossing(geometry: &Geometry, index: usize, ordinate: f32) -> Option<(f32, i32)> {
    match geometry.evaluation() {
        Evaluation::Quadratic => {
            let curve = &geometry.curves()[index];
            curve.crossing(ordinate).map(|x| (x, curve.direction))
        }
        Evaluation::Linear => {
            let [x, y, u, v] = geometry.edges()[index];
            if (y <= ordinate && v > ordinate) || (v <= ordinate && y > ordinate) {
                Some((
                    x + (ordinate - y) / (v - y) * (u - x),
                    i32::from(v > y) * 2 - 1,
                ))
            } else {
                None
            }
        }
    }
}
fn lower(mut range: Range<usize>, value: f32) -> usize {
    while range.start < range.end {
        let middle = range.start + (range.end - range.start) / 2;
        let sample = (middle / 4) as f32 + ((middle % 4) as f32 + 0.5) / 4.0;
        if sample < value {
            range.start = middle + 1;
        } else {
            range.end = middle;
        }
    }
    range.start
}
