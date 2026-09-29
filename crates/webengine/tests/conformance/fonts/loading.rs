use graphics::shared::{Backend, Cpu, Extent};
use textengine::{Face, Request as Font, Shaper};
use webengine::{
    Fault, Page, State,
    resource::{Kind, Provider, Request, Response},
};

struct Source {
    failed: bool,
    mime: &'static str,
    font: Vec<u8>,
    requests: Vec<String>,
}
impl Provider for Source {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        self.requests.push(request.url.to_string());
        let (mime, body) = match request.url.path() {
            "/page/index.html" => ("text/html", b"<link rel='stylesheet' href='../styles/first.css'><link rel='stylesheet' href='../other/second.css'><p>Office</p>".to_vec()),
            "/styles/first.css" => ("text/css", b"@font-face {font-family: first; src: url(../fonts/body.ttf#first);} p {font-family: first;}".to_vec()),
            "/other/second.css" => ("text/css", b"@font-face {font-family: second; src: url(../fonts/body.ttf#second);}".to_vec()),
            "/fonts/body.ttf" => {
                assert!(matches!(request.kind, Kind::Font));
                if self.failed { return Err(Fault("font request failed".into())); }
                (self.mime, self.font.clone())
            }
            _ => return Err(Fault("unexpected font URL".into())),
        };
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body,
        })
    }
}
fn source() -> Source {
    Source {
        failed: false,
        mime: "font/ttf",
        font: ::fixtures::bytes("Lato-Regular.ttf").to_vec(),
        requests: Vec::new(),
    }
}
fn page() -> Page {
    Page::new(
        "app://fonts/page/index.html",
        Extent {
            width: 240,
            height: 80,
        },
    )
    .unwrap()
}
#[test]
fn identity() {
    let mut source = source();
    let mut page = page();
    let catalog = page.fonts();
    let missing = catalog
        .shape(
            "A",
            &Font {
                families: vec!["first".into()].into(),
                ..Font::default()
            },
            0,
        )
        .unwrap();
    assert_eq!(missing.spans[0].font.face, Face::Missing);
    assert!(!missing.issues.is_empty());
    page.load(&mut source).unwrap();
    assert_eq!(
        source.requests,
        [
            "app://fonts/page/index.html",
            "app://fonts/styles/first.css",
            "app://fonts/other/second.css",
            "app://fonts/fonts/body.ttf"
        ]
    );
    let shape = |family: &str| {
        catalog
            .shape(
                "office",
                &Font {
                    families: vec![family.into()].into(),
                    ..Font::default()
                },
                0,
            )
            .unwrap()
    };
    let first = shape("first");
    assert_eq!(first.spans[0].font.face, shape("second").spans[0].font.face);
    let prepared = first.spans[0].font.prepare("office", 0).unwrap();
    let frame = page.frame().unwrap();
    let pixels = Cpu.render(&frame).unwrap();
    page.close();
    assert_eq!(Cpu.render(&frame).unwrap(), pixels);
    assert!(prepared.shape(0..6).unwrap().issues.is_empty());
}
#[test]
fn recovery() {
    let mut source = source();
    source.failed = true;
    let mut failed = page();
    assert!(failed.load(&mut source).is_err());
    assert_eq!(failed.inspect().state, State::Failed);
    assert!(failed.frame().is_none());
    failed.close();
    source.failed = false;
    page().load(&mut source).unwrap();
    source.mime = "image/png";
    assert!(page().load(&mut source).is_err());
    source.mime = "font/ttf";
    source.font = vec![0; 100];
    assert!(page().load(&mut source).is_err());
    source.font = vec![0; Kind::Font.limit() + 1];
    assert!(
        page()
            .load(&mut source)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
}
