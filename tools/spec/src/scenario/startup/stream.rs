use crate::Result;
use graphics::shared::{Backend, Cpu, Pass, State};
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct Report {
    scenario: String,
    backend: String,
    lanes: usize,
    rounds: usize,
    samples: Vec<u64>,
    total: u64,
    maximum: u32,
    unit: &'static str,
    endpoint: &'static str,
}

struct Context<'a> {
    scenario: &'a str,
    backend: &'a str,
    rounds: usize,
    lanes: usize,
}

pub fn run(scenario: &str, backend: &str, rounds: usize, lanes: usize) -> Result<Report> {
    if !(1..=8).contains(&lanes) || !(1..=1_000).contains(&rounds) {
        return Err("stream requires 1 through 8 lanes and 1 through 1000 rounds".into());
    }
    let (pass, _, _) = super::dispatch::prepare(scenario)?;
    match backend {
        "metal" => measure(
            Context {
                scenario,
                backend,
                rounds,
                lanes,
            },
            graphics::metal::Metal::new().map_err(error)?,
            pass,
        ),
        "executor" => measure(
            Context {
                scenario,
                backend,
                rounds,
                lanes,
            },
            graphics::metal::Executor::new(shared::Notify::new(|| {})).map_err(error)?,
            pass,
        ),
        _ => Err("stream backend must be metal or executor".into()),
    }
}

fn measure<B: Backend>(context: Context<'_>, mut backend: B, pass: Pass) -> Result<Report> {
    let mut warm = backend.submit(&pass).map_err(error)?;
    complete(&mut backend, std::slice::from_mut(&mut warm))?;
    let mut samples = Vec::with_capacity(context.rounds);
    let mut last = None;
    for _ in 0..context.rounds {
        let started = Instant::now();
        let mut work = (0..context.lanes)
            .map(|_| backend.submit(&pass).map_err(error))
            .collect::<Result<Vec<_>>>()?;
        complete(&mut backend, &mut work)?;
        samples.push(elapsed(started));
        last = work.pop();
    }
    let actual = backend
        .read(&last.ok_or("stream produced no work")?)
        .map_err(error)?;
    let expected = Cpu.submit(&pass).map_err(error)?;
    let maximum = super::super::rendering::difference(&actual, &expected);
    Ok(Report {
        scenario: context.scenario.into(),
        backend: context.backend.into(),
        lanes: context.lanes,
        rounds: context.rounds,
        total: samples.iter().sum(),
        samples,
        maximum,
        unit: "nanoseconds",
        endpoint: "warmed compute completion; preparation, readback and presentation excluded",
    })
}

fn complete<B: Backend>(backend: &mut B, work: &mut [B::Work]) -> Result<()> {
    let mut pending = vec![true; work.len()];
    while pending.iter().any(|pending| *pending) {
        for (work, pending) in work.iter_mut().zip(&mut pending) {
            if !*pending {
                continue;
            }
            match backend.poll(work).map_err(error)? {
                State::Pending => (),
                State::Complete => *pending = false,
                State::Failed(fault) => return Err(error(fault)),
            }
        }
        std::thread::yield_now();
    }
    Ok(())
}

fn elapsed(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
