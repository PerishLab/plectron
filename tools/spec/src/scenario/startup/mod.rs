use crate::Result;
use graphics::shared::{Backend, Evaluation, Output, Preparation, Rasterization};
use serde::Serialize;
use std::time::Instant;

mod dispatch;
mod stream;
pub use dispatch::run as dispatch;
pub use stream::run as stream;

#[derive(Serialize)]
pub struct First {
    pub scenario: String,
    pub device: u64,
    pub content: u64,
    pub preparation: u64,
    pub completion: u64,
    pub gpu: Option<u64>,
    pub first: u64,
    pub milestones: webengine::Milestones,
    pub setup: std::collections::BTreeMap<&'static str, u64>,
}
#[derive(Serialize)]
pub struct Checked {
    pub maximum: u32,
    pub checksum: u64,
    pub assets: std::collections::BTreeMap<String, super::source::Asset>,
}

pub fn run(id: &str, ready: impl FnOnce(&First) -> Result<()>) -> Result<Checked> {
    let case = super::load()?
        .into_iter()
        .find(|case| case.id == id)
        .ok_or("unknown startup scenario")?;
    let start = Instant::now();
    let mut metal = graphics::metal::Metal::new().map_err(error)?;
    let device = super::elapsed(start);
    let content = Instant::now();
    let mut source = super::source::Source::default();
    let extent = graphics::shared::Extent {
        width: case.viewport[0],
        height: case.viewport[1],
    };
    let mut page =
        webengine::Page::new(&format!("app://scenarios/{}", case.entry), extent).map_err(error)?;
    page.load(&mut source).map_err(error)?;
    let frame = page.frame().ok_or("startup produced no frame")?;
    let content = super::elapsed(content);
    let preparation = Instant::now();
    let mut renderer = Preparation::default();
    renderer.evaluation(Evaluation::Quadratic);
    renderer.rasterization(Rasterization::Local);
    let pass = renderer
        .prepare(&frame, Output::new(extent, 1.0).map_err(error)?)
        .map_err(error)?;
    let preparation = super::elapsed(preparation);
    let completion = Instant::now();
    let work = metal.submit(&pass).map_err(error)?;
    metal
        .wait(&work, std::time::Duration::from_secs(10))
        .map_err(error)?;
    let completion = super::elapsed(completion);
    page.acknowledge(frame.id()).map_err(error)?;
    let first = super::elapsed(start);
    let setup = metal.setup();
    ready(&First {
        scenario: id.into(),
        device,
        content,
        preparation,
        completion,
        gpu: work.duration().map(|duration| duration.as_nanos() as u64),
        first,
        milestones: page.inspect().milestones,
        setup: [
            ("driver", setup.driver),
            ("library", setup.library),
            ("states", setup.states),
        ]
        .map(|(name, duration)| (name, duration.as_nanos() as u64))
        .into(),
    })?;
    let actual = metal.read(&work).map_err(error)?;
    let reference = graphics::shared::Cpu.submit(&pass).map_err(error)?;
    let maximum = super::rendering::difference(&actual, &reference);
    if maximum > 1 {
        return Err("startup CPU/Metal difference exceeds one".into());
    }
    page.close();
    Ok(Checked {
        maximum,
        checksum: super::fingerprint(
            &actual
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_le_bytes())
                .collect::<Vec<_>>(),
        ),
        assets: source.assets().map_err(error)?,
    })
}
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
