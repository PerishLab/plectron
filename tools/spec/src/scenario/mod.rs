mod catalog;
mod check;
mod driver;
mod rendering;
mod report;
mod settings;
mod source;
#[cfg(target_os = "macos")]
pub mod startup;
mod summary;
use crate::Result;
pub use catalog::{Case, load, validate};
pub use report::Report;
pub(crate) use source::fingerprint;
use std::time::Instant;
pub use summary::Distribution;

pub struct Options<'a> {
    pub size: &'a str,
    pub mode: &'a str,
    pub backend: &'a str,
    pub platform: (&'static str, &'static str),
}
pub fn run(id: &str, options: Options<'_>) -> Result<Report> {
    configured(id, options, config::Store::default())
}
pub fn configured(id: &str, options: Options<'_>, config: config::Store) -> Result<Report> {
    let Options {
        size,
        mode,
        backend,
        platform,
    } = options;
    let mut case = load()?
        .into_iter()
        .find(|case| case.id == id)
        .ok_or("unknown scenario")?;
    let count = *case.sizes.get(size).ok_or("unknown scenario size")?;
    let repetitions = match mode {
        "--check" => 1,
        "--bench" => 3,
        _ => return Err("scenario requires --check or --bench".into()),
    };
    let settings = settings::Settings::read(&config, (count, repetitions))?;
    settings::viewport(&mut case, &config)?;
    let mut runs = Vec::new();
    let mut renderer = rendering::Renderer::new(backend, config.clone())?;
    for _ in 0..settings.warmups {
        let run = observe::call("warmup", || {
            driver::run(&case, &settings, &mut renderer, config.clone())
        })?;
        if let Some(failure) = run.failure {
            return Err(format!("warmup {}: {}", failure.action, failure.reason));
        }
    }
    for index in 0..settings.runs {
        if !settings.retained && (index > 0 || settings.warmups > 0) {
            renderer = rendering::Renderer::new(backend, config.clone())?;
        }
        runs.push(observe::call(format!("run.{index}"), || {
            driver::run(&case, &settings, &mut renderer, config.clone())
        })?);
    }
    let summary = summary::collect(&runs, settings.reference);
    Ok(Report {
        repetitions: settings.runs,
        case,
        count: settings.count,
        edits: settings.edits,
        size: size.into(),
        backend: backend.into(),
        mode: mode.into(),
        architecture: platform.0,
        system: platform.1,
        debug: cfg!(debug_assertions),
        unit: "nanoseconds",
        rasterization: "local-quadratic-4x4",
        endpoint: "render completion; no physical presentation measurement",
        accounting: "total sums evaluation, publication, preparation and completion; excludes diagnostics and readback",
        capture: "optional mutation invocation to encoded PNG available in process; includes readback/encoding, excludes file IO/reference verification and command transport; steady reuses retained PNG",
        summary,
        runs,
    })
}
fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}
