mod incremental;
use graphics::shared::{Backend, Cpu, Extent};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use webengine::{
    Fault, Page,
    resource::{Cancellation, Provider, Request, Response, Transport, Url},
};
struct File {
    mime: String,
    body: Vec<u8>,
}
struct Depot {
    files: BTreeMap<String, File>,
    requested: Mutex<Vec<String>>,
}
#[derive(Clone)]
struct Source(Arc<Depot>);
impl Source {
    fn get(&self, url: &Url) -> Result<Response, Fault> {
        self.0.requested.lock().unwrap().push(url.to_string());
        let file = self
            .0
            .files
            .get(url.path())
            .ok_or_else(|| Fault("missing image fixture".into()))?;
        Ok(Response {
            status: 200,
            mime: file.mime.clone(),
            body: file.body.clone(),
        })
    }
}
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if matches!(request.kind, webengine::resource::Kind::Font) {
            let name = request.url.path().rsplit('/').next().unwrap();
            return Ok(Response {
                status: 200,
                mime: "font/ttf".into(),
                body: ::fixtures::bytes(name).to_vec(),
            });
        }
        let response: Result<Response, Fault> = {
            let url = &request.url;
            Source::get(self, url)
        };
        let mut response = response?;
        if response.mime == "text/css" {
            response.body = ::fixtures::stylesheet(&response.body);
        }
        Ok(response)
    }
    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(Arc::new(self.clone()))
    }
}
impl Transport for Source {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        cancellation.check()?;
        self.get(&request.url)
    }
}
fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&color.repeat(width as usize * height as usize))
        .unwrap();
    bytes
}
fn source(mime: &str, body: Vec<u8>) -> Source {
    Source(Arc::new(Depot {files:BTreeMap::from([
        ("/index.html".into(),File {mime:"text/html".into(),body:b"<div id='root'><img id='first' src='./one.png'><img id='second' src='one.png#same'></div>".to_vec()}),
        ("/one.png".into(),File {mime:mime.into(),body}),
        ("/two.png".into(),File {mime:"image/png".into(),body:png(8,4,[0,0,255,128])}),
    ]),requested:Mutex::new(Vec::new())}))
}
#[test]
fn replacement() {
    let mut source = source("image/png", png(8, 4, [255, 0, 0, 255]));
    let mut page = Page::new(
        "app://images/index.html",
        Extent {
            width: 64,
            height: 64,
        },
    )
    .unwrap();
    page.load(&mut source).unwrap();
    page.evaluate(&fixtures::script("document", "")).unwrap();
    assert_eq!(source.0.requested.lock().unwrap().len(), 2);
    assert_eq!(page.inspect().resources.len(), 2);
    let original = page.frame().unwrap();
    let pixels = Cpu.render(&original).unwrap();
    page.acknowledge(original.id()).unwrap();
    page.evaluate("document.getElementById('first').setAttribute('src', 'two.png');")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().reuse.computed, 0);
    let changed = page.frame().unwrap();
    assert_ne!(Cpu.render(&changed).unwrap(), pixels);
    page.acknowledge(changed.id()).unwrap();
    page.evaluate("document.getElementById('first').setAttribute('src', 'missing.png');")
        .unwrap();
    assert!(page.render().is_err());
    page.evaluate("document.getElementById('first').setAttribute('src', './one.png');")
        .unwrap();
    page.render().unwrap();
    let restored = page.frame().unwrap();
    assert_eq!(Cpu.render(&restored).unwrap(), pixels);
    let calls = source.0.requested.lock().unwrap();
    assert_eq!(
        calls.iter().filter(|url| url.ends_with("one.png")).count(),
        1
    );
    drop(calls);
    page.close();
    drop(source);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}
#[test]
fn rejection() {
    let cases = [
        ("text/plain", png(1, 1, [0, 0, 0, 255])),
        ("image/png", b"invalid".to_vec()),
        ("image/png", png(1025, 1024, [0, 0, 0, 255])),
    ];
    for (mime, body) in cases {
        let mut page = Page::new(
            "app://images/index.html",
            Extent {
                width: 32,
                height: 32,
            },
        )
        .unwrap();
        assert!(page.load(&mut source(mime, body)).is_err());
        assert!(page.frame().is_none());
    }
}
