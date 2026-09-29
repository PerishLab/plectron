mod report;
mod source;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Output, Pacing, Preparation, Pulse, State};
use report::{Capture, Sample, Timings};
use std::time::Instant;
use webengine::{Fault, Page};

const ITERATIONS: usize = 64;
const WARMUP: usize = 8;

#[derive(Clone, Copy)]
struct Case {
    id: &'static str,
    width: usize,
    height: usize,
    scale: f64,
}

struct Run {
    case: Case,
}

pub fn run(platform: (&'static str, &'static str)) -> crate::Result<report::Benchmark> {
    measure(platform).map_err(|fault| fault.to_string())
}

fn measure(platform: (&'static str, &'static str)) -> Result<report::Benchmark, Fault> {
    let cases = [
        Case {
            id: "isolated",
            width: 1280,
            height: 160,
            scale: 2.0,
        },
        Case {
            id: "todo",
            width: 760,
            height: 680,
            scale: 2.0,
        },
    ];
    let mut samples = Vec::new();
    for repetition in 0..3 {
        for case in cases {
            samples.push(Run { case }.cpu(repetition)?);
            samples.push(Run { case }.metal(repetition)?);
        }
    }
    report::Benchmark::new(platform, &cases, samples)
}

impl Run {
    fn cpu(self, repetition: usize) -> Result<Sample, Fault> {
        let mut page = self.case.page()?;
        let mut preparation = Preparation::default();
        let mut backend = Cpu;
        self.warm(&mut page, &mut preparation, &mut backend)?;
        let before = page.inspect();
        let mut timings = Timings::default();
        let mut checksums = Vec::new();
        for _ in 0..ITERATIONS {
            let (frame, pass) = self.advance(&mut page, &mut preparation, &mut timings)?;
            let start = Instant::now();
            let mut work = backend.submit(&pass).map_err(error)?;
            timings.submission.push(super::super::elapsed(start));
            let start = Instant::now();
            complete(&mut backend, &mut work)?;
            timings.completion.push(super::super::elapsed(start));
            checksums.push(report::checksum(&work.pixels));
            acknowledge(&mut page, frame, &mut timings)?;
        }
        Capture {
            case: self.case,
            backend: "cpu",
            repetition,
            device: None,
            timings,
            before,
            after: page.inspect(),
            checksums,
            maximum: 0,
        }
        .sample()
    }

    fn metal(self, repetition: usize) -> Result<Sample, Fault> {
        let mut page = self.case.page()?;
        let mut preparation = Preparation::default();
        let mut backend = Metal::new().map_err(error)?;
        let device = backend.name();
        self.warm(&mut page, &mut preparation, &mut backend)?;
        let before = page.inspect();
        let mut timings = Timings::default();
        let mut checksums = Vec::new();
        let mut maximum = 0;
        for _ in 0..ITERATIONS {
            let (frame, pass) = self.advance(&mut page, &mut preparation, &mut timings)?;
            let start = Instant::now();
            let mut work = backend.submit(&pass).map_err(error)?;
            timings.submission.push(super::super::elapsed(start));
            let start = Instant::now();
            complete(&mut backend, &mut work)?;
            timings.completion.push(super::super::elapsed(start));
            let duration = work
                .duration()
                .ok_or_else(|| Fault("Metal did not report GPU duration".into()))?;
            timings
                .gpu
                .push(duration.as_nanos().min(u128::from(u64::MAX)) as u64);
            let image = backend.read(&work).map_err(error)?;
            let expected = Cpu.submit(&pass).map_err(error)?;
            maximum = maximum.max(report::difference(&expected.pixels, &image.pixels)?);
            checksums.push(report::checksum(&expected.pixels));
            acknowledge(&mut page, frame, &mut timings)?;
        }
        Capture {
            case: self.case,
            backend: "metal",
            repetition,
            device: Some(device),
            timings,
            before,
            after: page.inspect(),
            checksums,
            maximum,
        }
        .sample()
    }

    fn warm<B: Backend>(
        &self,
        page: &mut Page,
        preparation: &mut Preparation,
        backend: &mut B,
    ) -> Result<(), Fault> {
        for _ in 0..WARMUP {
            let mut timings = Timings::default();
            let (frame, pass) = self.advance(page, preparation, &mut timings)?;
            let mut work = backend.submit(&pass).map_err(error)?;
            complete(backend, &mut work)?;
            page.acknowledge(frame)?;
        }
        Ok(())
    }

    fn advance(
        &self,
        page: &mut Page,
        preparation: &mut Preparation,
        timings: &mut Timings,
    ) -> Result<(u64, graphics::shared::Pass), Fault> {
        let deadline = match page.pacing() {
            Pacing::At(deadline) => deadline,
            _ => return Err(Fault("caret animation has no deadline".into())),
        };
        let start = Instant::now();
        if !page.pulse(Pulse::new(deadline))? {
            return Err(Fault("caret deadline produced no effect".into()));
        }
        timings.pulse.push(super::super::elapsed(start));
        let start = Instant::now();
        let frame = page
            .frame()
            .ok_or_else(|| Fault("caret effect produced no frame".into()))?;
        timings.frame.push(super::super::elapsed(start));
        let id = frame.id();
        let start = Instant::now();
        let output = Output::new(frame.extent(), self.case.scale).map_err(error)?;
        let pass = preparation.prepare(&frame, output).map_err(error)?;
        timings.preparation.push(super::super::elapsed(start));
        Ok((id, pass))
    }
}

fn acknowledge(page: &mut Page, frame: u64, timings: &mut Timings) -> Result<(), Fault> {
    let start = Instant::now();
    page.acknowledge(frame)?;
    timings.acknowledge.push(super::super::elapsed(start));
    Ok(())
}

fn complete<B: Backend>(backend: &mut B, work: &mut B::Work) -> Result<(), Fault> {
    loop {
        match backend.poll(work).map_err(error)? {
            State::Complete => return Ok(()),
            State::Failed(fault) => return Err(error(fault)),
            State::Pending => std::thread::yield_now(),
        }
    }
}

fn error(value: impl std::fmt::Display) -> Fault {
    Fault(value.to_string())
}
