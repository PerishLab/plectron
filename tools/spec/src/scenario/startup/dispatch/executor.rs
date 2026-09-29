use super::{Context, Outcome, Selection, elapsed, error, prepare, report};
use crate::Result;
use graphics::shared::{Backend, Cpu, State};
use std::time::Instant;

pub(super) fn run(entry: &str) -> Result<super::Report> {
    let epoch = Instant::now();
    let mut metal = graphics::metal::Executor::new(shared::Notify::new(|| {})).map_err(error)?;
    let (pass, content, preparation) = prepare(entry)?;
    let build = super::Build {
        content,
        preparation,
        ready: elapsed(epoch),
    };
    let start = elapsed(epoch);
    let mut work = metal.submit(&pass).map_err(error)?;
    loop {
        match metal.poll(&mut work).map_err(error)? {
            State::Pending => std::thread::yield_now(),
            State::Complete => break,
            State::Failed(fault) => return Err(error(fault)),
        }
    }
    let finish = elapsed(epoch);
    let actual = metal.read(&work).map_err(error)?;
    let expected = Cpu.submit(&pass).map_err(error)?;
    let maximum = super::super::super::rendering::difference(&actual, &expected);
    Ok(report(
        Context {
            scenario: entry,
            policy: "executor",
            delay: 0,
            build,
        },
        Selection {
            metal: Some(Outcome {
                report: super::Attempt::new(None, start, finish),
                image: actual,
            }),
            cpu: None,
            winner: "metal",
        },
    )
    .checked(Some(maximum)))
}
