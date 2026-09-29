use super::{Sample, elapsed};
use crate::Result;
use graphics::shared::{Backend, Cpu};
use jsengine::jit::Mode;
use std::hint::black_box;
use std::time::Instant;
use webengine::resource::{Provider, Response};
use webengine::{Fault, Page};

struct Source;
impl Provider for Source {
    fn request(
        &mut self,
        request: &webengine::resource::Request,
    ) -> std::result::Result<Response, Fault> {
        let url = &request.url;
        let (mime, body) = match url.path() {
            "/index.html" => (
                "text/html",
                "<html><head><script src='app.js'></script></head><body><button id='count'>Count</button></body></html>".into(),
            ),
            "/app.js" => (
                "text/javascript",
                fixtures::script(
                    "document, bridge",
                    "const calculate = (x, y) => x * y + 2; let count = 0; const button = document.getElementById('count'); bridge.onMessage(message => { count += 1; button.setText('Count: ' + calculate(count, 3)); });",
                ),
            ),
            _ => return Err(Fault("unexpected benchmark resource".into())),
        };
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body: body.as_bytes().to_vec(),
        })
    }
}

pub(super) fn sample(mode: Mode, repetition: usize) -> Result<Sample> {
    measure(mode, repetition).map_err(|error| error.to_string())
}

fn measure(mode: Mode, repetition: usize) -> std::result::Result<Sample, Fault> {
    let start = Instant::now();
    let mut page = Page::new(
        "app://benchmark/index.html",
        domengine::Extent {
            width: 160,
            height: 64,
        },
    )?;
    page.load(&mut Source)?;
    page.tier(mode)?;
    present(&mut page)?;
    let startup = elapsed(start);
    let start = Instant::now();
    operation(&mut page)?;
    let cold = elapsed(start);
    for _ in 0..32 {
        operation(&mut page)?;
    }
    let start = Instant::now();
    for _ in 0..128 {
        operation(&mut page)?;
    }
    let elapsed = elapsed(start);
    if page.evaluate("count;")? != "161" {
        return Err(Fault("application benchmark result mismatch".into()));
    }
    Ok(Sample {
        scenario: "message-dom-layout-scene-cpu".into(),
        repetition,
        startup,
        cold,
        iterations: 128,
        elapsed,
        report: page.inspect().performance,
    })
}

fn operation(page: &mut Page) -> std::result::Result<(), Fault> {
    page.receive("tick".into())?;
    page.render()?;
    present(page)
}

fn present(page: &mut Page) -> std::result::Result<(), Fault> {
    let frame = page
        .frame()
        .ok_or_else(|| Fault("benchmark did not produce frame".into()))?;
    let image = Cpu
        .render(&frame)
        .map_err(|error| Fault(error.to_string()))?;
    black_box(image);
    page.acknowledge(frame.id())?;
    Ok(())
}
