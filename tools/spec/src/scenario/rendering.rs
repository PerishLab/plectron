use super::report::Rendering;
use graphics::shared::{
    Backend, Cpu, Evaluation, Frame, Image, Output, Preparation, Rasterization,
};
use serde::Serialize;
use std::io::Write;
use std::time::Instant;

pub(super) struct Renderer {
    backend: Device,
    preparation: Preparation,
    capturing: bool,
    directory: Option<std::path::PathBuf>,
    retained: Option<(Capture, Vec<u8>)>,
}
#[derive(Clone, Serialize)]
pub struct Capture {
    pub ready: u64,
    pub encoding: u64,
    pub bytes: usize,
    pub frame: u64,
    pub file: Option<String>,
}
enum Device {
    Cpu,
    #[cfg(target_os = "macos")]
    Metal(Box<graphics::metal::Metal>),
}
impl Renderer {
    pub fn new(name: &str, config: config::Store) -> crate::Result<Self> {
        let capturing = config
            .get("scenario.capture")
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(false);
        let directory = config
            .get("scenario.output")
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| e.to_string())?;
        let backend = match name {
            "cpu" => Device::Cpu,
            #[cfg(target_os = "macos")]
            "metal" => Device::Metal(Box::new(
                graphics::metal::Metal::new().map_err(|e| e.to_string())?,
            )),
            _ => return Err("unknown scenario backend".into()),
        };
        let mut preparation = Preparation::configured(config);
        preparation.evaluation(Evaluation::Quadratic);
        preparation.rasterization(Rasterization::Local);
        Ok(Self {
            backend,
            preparation,
            capturing,
            directory,
            retained: None,
        })
    }
    pub fn render(
        &mut self,
        frame: &Frame,
        begin: Instant,
    ) -> Result<(Rendering, Option<Capture>), graphics::shared::Fault> {
        let mut report = Rendering::default();
        let before = self.preparation.rasterizer().report();
        let start = Instant::now();
        let pass = self
            .preparation
            .prepare(frame, Output::new(frame.extent(), 1.0)?)?;
        report.preparation = super::elapsed(start);
        report.groups = pass.layer().groups();
        report.work = self.preparation.work();
        report.bounds = Some(self.preparation.bounds());
        report.pruning = Some(self.preparation.groups());
        report.builds = self.preparation.rasterizer().report().builds - before.builds;
        let start = Instant::now();
        let actual = match &mut self.backend {
            Device::Cpu => {
                let image = Cpu.submit(&pass)?;
                report.submission = super::elapsed(start);
                report.completion = report.submission;
                image
            }
            #[cfg(target_os = "macos")]
            Device::Metal(metal) => submit(metal, &pass, &mut report)?,
        };
        let capture = if self.capturing {
            Some(self.capture(&actual, frame.id(), begin)?)
        } else {
            None
        };
        if !matches!(self.backend, Device::Cpu) {
            let expected = observe::call("verification.cpu", || Cpu.submit(&pass))?;
            report.maximum = difference(&actual, &expected);
            if report.maximum > 1 {
                return Err(graphics::shared::Fault(
                    "scenario CPU/Metal difference exceeds one".into(),
                ));
            }
        }
        report.checksum = actual
            .pixels
            .iter()
            .fold(0_u64, |h, p| h.wrapping_mul(1099511628211) ^ u64::from(*p));
        Ok((report, capture))
    }
    fn capture(
        &mut self,
        image: &Image,
        frame: u64,
        begin: Instant,
    ) -> Result<Capture, graphics::shared::Fault> {
        let start = Instant::now();
        let mut bytes = Vec::new();
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|pixel| [16, 8, 0].map(|shift| (pixel >> shift) as u8))
            .collect();
        {
            let mut encoder = png::Encoder::new(
                &mut bytes,
                image.extent.width as u32,
                image.extent.height as u32,
            );
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(fault)?;
            writer.write_image_data(&pixels).map_err(fault)?;
            writer.finish().map_err(fault)?;
        }
        let encoding = super::elapsed(start);
        let ready = super::elapsed(begin);
        let file = self
            .directory
            .as_ref()
            .map(|root| root.join(format!("{frame}.png")));
        if let Some(file) = &file {
            std::fs::File::create_new(file)
                .and_then(|mut file| file.write_all(&bytes))
                .map_err(fault)?;
        }
        let capture = Capture {
            ready,
            encoding,
            bytes: bytes.len(),
            frame,
            file: file.map(|path| path.to_string_lossy().into_owned()),
        };
        self.retained = Some((capture.clone(), bytes));
        Ok(capture)
    }
    pub fn repeat(&self, begin: Instant) -> Option<Capture> {
        self.retained.as_ref().map(|(capture, _)| Capture {
            ready: super::elapsed(begin),
            encoding: 0,
            ..capture.clone()
        })
    }
}
fn fault(error: impl std::fmt::Display) -> graphics::shared::Fault {
    graphics::shared::Fault(error.to_string())
}
pub(super) fn difference(a: &Image, b: &Image) -> u32 {
    assert_eq!(a.extent, b.extent);
    a.pixels
        .iter()
        .zip(&b.pixels)
        .flat_map(|(a, b)| [0, 8, 16].map(|s| ((a >> s) & 255).abs_diff((b >> s) & 255)))
        .max()
        .unwrap_or(0)
}
#[cfg(target_os = "macos")]
fn submit(
    metal: &mut graphics::metal::Metal,
    pass: &graphics::shared::Pass,
    report: &mut Rendering,
) -> Result<Image, graphics::shared::Fault> {
    let textures = metal.inspect(graphics::metal::Resource::Textures).uploads;
    let uploads = metal.inspect(graphics::metal::Resource::Coverages).uploads;
    let start = Instant::now();
    let work = metal.submit(pass)?;
    report.submission = super::elapsed(start);
    metal.wait(&work, std::time::Duration::from_secs(10))?;
    report.completion = super::elapsed(start);
    report.gpu = work.duration().map(|d| d.as_nanos() as u64);
    report.uploads = Some(metal.inspect(graphics::metal::Resource::Coverages).uploads - uploads);
    report.textures = Some(metal.inspect(graphics::metal::Resource::Textures).uploads - textures);
    let start = Instant::now();
    let image = metal.read(&work)?;
    report.readback = super::elapsed(start);
    Ok(image)
}
