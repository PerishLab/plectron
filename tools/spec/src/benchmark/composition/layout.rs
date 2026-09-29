use super::Sample;
use crate::Result;
use ::fixtures::Text;
use domengine::{Constraints, Extent, Input, Pipeline, Sheet, html};
use std::{cell::Cell, hint::black_box, time::Instant};
pub(super) fn sample(repetition: usize) -> Result<Sample> {
    run(repetition).map_err(|fault| fault.to_string())
}
fn run(repetition: usize) -> std::result::Result<Sample, domengine::Fault> {
    let document = html::parse(&format!(
        "<p>{}</p>",
        "An office with affinity. بِلَا بداية جميلة. ".repeat(3)
    ))?;
    let sheet = Sheet::parse(
        "p {font-family: lato, noto-arabic; font-size: 20px; overflow-wrap: anywhere;}",
    )?;
    let input = Input::capture(&document, &sheet)?;
    let preparation = Cell::new(0);
    let mut pipeline = Pipeline::new(Meter(&preparation));
    pipeline.enable(false);
    black_box(pipeline.build(
        &input,
        Constraints::viewport(Extent {
            width: 200,
            height: 400,
        }),
    )?);
    let mut shaping = domengine::Shaping::default();
    preparation.set(0);
    let start = Instant::now();
    for index in 0..16 {
        black_box(pipeline.build(
            &input,
            Constraints::viewport(Extent {
                width: 240 + index % 8 * 13,
                height: 400,
            }),
        )?);
        let report = pipeline.inspect().shaping;
        shaping.preparations += report.preparations;
        shaping.shapes += report.shapes;
        shaping.bytes += report.bytes;
        shaping.candidates += report.candidates;
        shaping.materialized += report.materialized;
    }
    Ok(Sample {
        scenario: "paragraph-widths".into(),
        repetition,
        iterations: 16,
        elapsed: super::super::elapsed(start),
        shaping: Some(shaping),
        preparation: preparation.get(),
        ..Sample::default()
    })
}

struct Meter<'a>(&'a Cell<u64>);
impl domengine::Shaper for Meter<'_> {
    fn revision(&self) -> u64 {
        Text.revision()
    }
    fn select(
        &self,
        source: &str,
        request: &textengine::Request,
        spacing: usize,
    ) -> std::result::Result<Vec<textengine::Selection>, textengine::Fault> {
        Text.select(source, request, spacing)
    }
    fn prepare(
        &self,
        source: &str,
        request: &textengine::Request,
        spacing: usize,
    ) -> std::result::Result<textengine::Prepared, textengine::Fault> {
        let start = Instant::now();
        let result = Text.prepare(source, request, spacing);
        self.0.set(self.0.get() + super::super::elapsed(start));
        result
    }
}
