use crate::ipc::error;
use graphics::shared::{Backend, Cpu, Extent, Image, Queue};
use plectron::{contract::Fault, resource::Services};
use std::{path::Path, time::Duration};
use webengine::Page;

pub fn run(backend: &str, source: (&Path, &str), output: &Path, scale: f64) -> Result<(), Fault> {
    let output = (
        output,
        graphics::shared::Output::new(
            Extent {
                width: 720,
                height: 640,
            },
            scale,
        )
        .map_err(error)?,
    );
    let mut services = Services::default();
    let service = services.serve(source.0)?;
    services.scheme(service, "app://capture/")?;
    let mut page = Page::new(&format!("app://capture/{}", source.1), output.1.logical())?;
    page.load(&mut services)?;
    let frame = page
        .frame()
        .ok_or_else(|| Fault("page produced no frame".into()))?;
    let (image, elapsed) = match backend {
        "cpu" => render(Cpu, &frame, output.1)?,
        #[cfg(target_os = "macos")]
        "metal" => render(
            graphics::metal::Metal::new().map_err(error)?,
            &frame,
            output.1,
        )?,
        _ => return Err(Fault("renderer must be cpu or supported metal".into())),
    };
    eprintln!(
        "[capture] scale={} logical={}x{} pixels={}x{} render/readback={:.3}ms",
        output.1.scale(),
        output.1.logical().width,
        output.1.logical().height,
        image.extent.width,
        image.extent.height,
        elapsed.as_secs_f64() * 1000.0
    );
    let mut bytes =
        format!("P6\n{} {}\n255\n", image.extent.width, image.extent.height).into_bytes();
    for pixel in image.pixels {
        bytes.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
    }
    std::fs::write(output.0, bytes).map_err(error)?;
    println!("{}", output.0.display());
    Ok(())
}

fn render(
    backend: impl Backend,
    frame: &graphics::shared::Frame,
    output: graphics::shared::Output,
) -> Result<(Image, Duration), Fault> {
    let started = std::time::Instant::now();
    let mut queue = Queue::new(backend);
    let target = queue.output(output).map_err(error)?;
    let ticket = queue.submit(target, frame).map_err(error)?;
    queue.wait(ticket, Duration::from_secs(5)).map_err(error)?;
    let image = queue.read(ticket).map_err(error)?;
    queue.release(ticket).map_err(error)?;
    queue.close(target).map_err(error)?;
    Ok((image, started.elapsed()))
}
