use super::Sample;
use crate::Result;
use graphics::shared::{Backend, Cpu};
use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};
use webengine::{
    Fault, Page,
    resource::{Cancellation, Provider, Request, Response, Transport},
};
struct Source;
fn picture(index: usize) -> std::result::Result<Vec<u8>, Fault> {
    static PICTURES: OnceLock<crate::Result<[Vec<u8>; 2]>> = OnceLock::new();
    let pictures = PICTURES
        .get_or_init(|| {
            let application = fixtures::Application::new("pictures");
            Ok([
                application.load("/dawn.png")?.body,
                application.load("/dusk.png")?.body,
            ])
        })
        .as_ref()
        .map_err(|error| Fault(error.clone()))?;
    Ok(pictures[index].clone())
}
impl Provider for Source {
    fn request(
        &mut self,
        request: &webengine::resource::Request,
    ) -> std::result::Result<Response, Fault> {
        let url = &request.url;
        let (mime, body) = match url.path() {
            "/index.html" => (
                "text/html",
                format!(
                    "<div id='root'><img id='image' src='one.png'>{}</div>",
                    "<span>text</span>".repeat(256)
                )
                .into_bytes(),
            ),
            "/one.png" => ("image/png", picture(0)?),
            "/two.png" => ("image/png", picture(1)?),
            _ => return Err(Fault("unknown benchmark resource".into())),
        };
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body,
        })
    }
    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(Arc::new(Source))
    }
}
impl Transport for Source {
    fn send(
        &self,
        request: &Request,
        cancellation: &Cancellation,
    ) -> std::result::Result<Response, Fault> {
        cancellation.check()?;
        Source.get(&request.url)
    }
}
pub(super) fn sample(scenario: &str, repetition: usize) -> Result<Sample> {
    run(scenario, repetition).map_err(|fault| fault.to_string())
}
fn run(scenario: &str, repetition: usize) -> std::result::Result<Sample, Fault> {
    let mut page = Page::new(
        "app://bench/index.html",
        graphics::shared::Extent {
            width: 320,
            height: 240,
        },
    )?;
    page.load(&mut Source)?;
    page.evaluate("const image = document.getElementById('image'); const root = document.getElementById('root');")?;
    update(&mut page, scenario, 0)?;
    let before = page.inspect().images;
    let start = Instant::now();
    for index in 1..=16 {
        update(&mut page, scenario, index)?;
    }
    let elapsed = super::super::elapsed(start);
    let mut images = page.inspect().images;
    images.scans -= before.scans;
    images.visited -= before.visited;
    images.changes -= before.changes;
    images.candidates -= before.candidates;
    images.fetches -= before.fetches;
    images.decoded -= before.decoded;
    images.bindings -= before.bindings;
    let mut fresh = Page::new(
        "app://bench/index.html",
        graphics::shared::Extent {
            width: 320,
            height: 240,
        },
    )?;
    fresh.load(&mut Source)?;
    fresh.evaluate("const image = document.getElementById('image'); const root = document.getElementById('root');")?;
    update(&mut fresh, scenario, 16)?;
    let expected = Cpu
        .render(&fresh.frame().unwrap())
        .map_err(|fault| Fault(fault.to_string()))?;
    let actual = Cpu
        .render(&page.frame().unwrap())
        .map_err(|fault| Fault(fault.to_string()))?;
    if expected != actual {
        return Err(Fault("page benchmark differs from fresh output".into()));
    }
    Ok(Sample {
        scenario: format!("page-{scenario}"),
        repetition,
        iterations: 16,
        elapsed,
        images: Some(images),
        ..Sample::default()
    })
}
fn update(page: &mut Page, scenario: &str, index: usize) -> std::result::Result<(), Fault> {
    let source = match (scenario, index.is_multiple_of(2)) {
        ("swap", true) => "image.setAttribute('src', 'one.png');",
        ("swap", false) => "image.setAttribute('src', 'two.png');",
        ("color", true) => "root.setAttribute('style', 'color: #aa2200;');",
        ("color", false) => "root.setAttribute('style', 'color: #0022aa;');",
        _ => "0;",
    };
    page.evaluate(source)?;
    page.render()
}
