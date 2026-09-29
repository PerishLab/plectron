use crate::Result;
use graphics::shared::{Backend, Cpu, Evaluation, Image, Output, Pass, Preparation, Rasterization};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

mod executor;
mod model;
pub use model::Report;
use model::{Attempt, Build};

struct Outcome {
    report: Attempt,
    image: Image,
}

struct Context<'a> {
    scenario: &'a str,
    policy: &'a str,
    delay: u64,
    build: Build,
}

struct Selection {
    metal: Option<Outcome>,
    cpu: Option<Outcome>,
    winner: &'static str,
}

pub fn run(scenario: &str, policy: &str, delay: u64) -> Result<Report> {
    match policy {
        "serial" => serial(scenario),
        "cpu" => cpu(scenario),
        "overlap" => parallel(scenario, policy, None),
        "executor" => executor::run(scenario),
        "hedge" => parallel(scenario, policy, Some(delay)),
        _ => Err("dispatch policy must be serial, cpu, overlap, executor or hedge".into()),
    }
}

fn serial(entry: &str) -> Result<Report> {
    let epoch = Instant::now();
    let device = Instant::now();
    let mut metal = graphics::metal::Metal::new().map_err(error)?;
    let device = elapsed(device);
    let (pass, content, preparation) = prepare(entry)?;
    let build = Build {
        content,
        preparation,
        ready: elapsed(epoch),
    };
    let start = elapsed(epoch);
    let work = metal.submit(&pass).map_err(error)?;
    metal.wait(&work, Duration::from_secs(10)).map_err(error)?;
    let finish = elapsed(epoch);
    let actual = metal.read(&work).map_err(error)?;
    let expected = Cpu.submit(&pass).map_err(error)?;
    let maximum = super::super::rendering::difference(&actual, &expected);
    Ok(report(
        Context {
            scenario: entry,
            policy: "serial",
            delay: 0,
            build,
        },
        Selection {
            metal: Some(Outcome {
                report: Attempt::new(Some(device), start, finish),
                image: actual,
            }),
            cpu: None,
            winner: "metal",
        },
    )
    .checked(Some(maximum)))
}

fn cpu(entry: &str) -> Result<Report> {
    let epoch = Instant::now();
    let (pass, content, preparation) = prepare(entry)?;
    let build = Build {
        content,
        preparation,
        ready: elapsed(epoch),
    };
    let start = elapsed(epoch);
    let image = Cpu.submit(&pass).map_err(error)?;
    let finish = elapsed(epoch);
    Ok(report(
        Context {
            scenario: entry,
            policy: "cpu",
            delay: 0,
            build,
        },
        Selection {
            metal: None,
            cpu: Some(Outcome {
                report: Attempt::new(None, start, finish),
                image,
            }),
            winner: "cpu",
        },
    ))
}

fn parallel(name: &str, policy: &str, delay: Option<u64>) -> Result<Report> {
    let epoch = Instant::now();
    let winner = Arc::new(AtomicU8::new(0));
    let (sender, receiver) = mpsc::sync_channel::<Arc<Pass>>(1);
    let selected = winner.clone();
    let metal = std::thread::spawn(move || -> Result<Outcome> {
        let device = Instant::now();
        let mut metal = graphics::metal::Metal::new().map_err(error)?;
        let device = elapsed(device);
        let pass = receiver.recv().map_err(error)?;
        let start = elapsed(epoch);
        let work = metal.submit(&pass).map_err(error)?;
        metal.wait(&work, Duration::from_secs(10)).map_err(error)?;
        let finish = elapsed(epoch);
        selected
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .ok();
        let image = metal.read(&work).map_err(error)?;
        Ok(Outcome {
            report: Attempt::new(Some(device), start, finish),
            image,
        })
    });
    let cpu = delay.map(|delay| {
        let (sender, receiver) = mpsc::sync_channel::<(Arc<Pass>, u64)>(1);
        let winner = winner.clone();
        let thread = std::thread::spawn(move || -> Result<Option<Outcome>> {
            let (pass, ready) = receiver.recv().map_err(error)?;
            let deadline = epoch + Duration::from_nanos(ready.saturating_add(delay));
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            if winner.load(Ordering::Acquire) != 0 {
                return Ok(None);
            }
            let start = elapsed(epoch);
            let image = Cpu.submit(&pass).map_err(error)?;
            let finish = elapsed(epoch);
            winner
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                .ok();
            Ok(Some(Outcome {
                report: Attempt::new(None, start, finish),
                image,
            }))
        });
        (sender, thread)
    });
    let (pass, content, preparation) = prepare(name)?;
    let ready = elapsed(epoch);
    let build = Build {
        content,
        preparation,
        ready,
    };
    let pass = Arc::new(pass);
    sender.send(pass.clone()).map_err(error)?;
    if let Some((sender, _)) = &cpu {
        sender.send((pass.clone(), ready)).map_err(error)?;
    }
    let metal = metal.join().map_err(|_| "Metal experiment panicked")??;
    let cpu = cpu
        .map(|(_, thread)| thread.join().map_err(|_| "CPU experiment panicked"))
        .transpose()?
        .transpose()?
        .flatten();
    let chosen = match winner.load(Ordering::Acquire) {
        1 => "metal",
        2 => "cpu",
        _ => return Err("dispatch experiment produced no winner".into()),
    };
    let maximum = match &cpu {
        Some(cpu) => super::super::rendering::difference(&metal.image, &cpu.image),
        None => {
            let expected = Cpu.submit(&pass).map_err(error)?;
            super::super::rendering::difference(&metal.image, &expected)
        }
    };
    Ok(report(
        Context {
            scenario: name,
            policy,
            delay: delay.unwrap_or(0),
            build,
        },
        Selection {
            metal: Some(metal),
            cpu,
            winner: chosen,
        },
    )
    .checked(Some(maximum)))
}

pub(super) fn prepare(name: &str) -> Result<(Pass, u64, u64)> {
    let case = super::super::load()?
        .into_iter()
        .find(|case| case.id == name)
        .ok_or("unknown dispatch scenario")?;
    let content = Instant::now();
    let extent = graphics::shared::Extent {
        width: case.viewport[0],
        height: case.viewport[1],
    };
    let mut source = super::super::source::Source::default();
    let mut page =
        webengine::Page::new(&format!("app://scenarios/{}", case.entry), extent).map_err(error)?;
    page.load(&mut source).map_err(error)?;
    let frame = page.frame().ok_or("dispatch scenario produced no frame")?;
    let content = elapsed(content);
    let started = Instant::now();
    let mut renderer = Preparation::default();
    renderer.evaluation(Evaluation::Quadratic);
    renderer.rasterization(Rasterization::Local);
    let pass = renderer
        .prepare(&frame, Output::new(extent, 1.0).map_err(error)?)
        .map_err(error)?;
    let preparation = elapsed(started);
    page.close();
    Ok((pass, content, preparation))
}

fn report(context: Context<'_>, selection: Selection) -> Report {
    let selected = match selection.winner {
        "metal" => {
            selection
                .metal
                .as_ref()
                .expect("selected Metal attempt")
                .report
                .finish
        }
        "cpu" => {
            selection
                .cpu
                .as_ref()
                .expect("selected CPU attempt")
                .report
                .finish
        }
        _ => unreachable!(),
    };
    let duplicate = selection.metal.is_some() && selection.cpu.is_some();
    Report {
        scenario: context.scenario.into(),
        policy: context.policy.into(),
        delay: context.delay,
        content: context.build.content,
        preparation: context.build.preparation,
        ready: context.build.ready,
        metal: selection.metal.map(|attempt| attempt.report),
        cpu: selection.cpu.map(|attempt| attempt.report),
        winner: selection.winner.into(),
        selected,
        duplicate,
        maximum: None,
        unit: "nanoseconds",
        endpoint: "first completed compute artifact; excludes readback and presentation",
        accounting: "content and preparation run once; competing attempts consume the same immutable Pass",
    }
}

fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
