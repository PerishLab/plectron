use super::{Extent, Fault, Page, Provider, Response, Source, State, loaded, source};

#[test]
fn passage() {
    let mut page = loaded(&mut source());
    page.evaluate(r"const label = 'A\u{1f600}e\u{301}中'; button.setText(label.slice(1, 4)); bridge.postMessage(label);").unwrap();
    page.render().unwrap();
    let expected = "😀e\u{301}";
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == expected)
    );
    assert_eq!(page.evaluate("label.length;").unwrap(), "5");
    assert_eq!(page.messages(), ["A😀e\u{301}中"]);
    page.receive("👨‍👩‍👧‍👦".into()).unwrap();
    page.render().unwrap();
    assert!(page.inspect().nodes.iter().any(|node| node.content == "👨‍👩‍👧‍👦"));
    page.evaluate("bridge.onMessage(function (text) { button.setText(text.slice(1, 2)); bridge.postMessage(text.length + ':' + text.at(-1)); });").unwrap();
    page.receive("A😀中".into()).unwrap();
    page.render().unwrap();
    assert!(page.inspect().nodes.iter().any(|node| node.content == "😀"));
    assert_eq!(page.messages(), ["3:中"]);
    let frame = page.frame().unwrap();
    let image = graphics::shared::Backend::render(&mut graphics::shared::Cpu, &frame).unwrap();
    assert_eq!(image.pixels.len(), 24_000);
}

struct Invalid {
    source: Source,
    path: &'static str,
    bytes: Vec<u8>,
}

impl Provider for Invalid {
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
            let mut response = self.source.get(url)?;
            if url.path() == self.path {
                response.body = self.bytes.clone();
            }
            Ok(response)
        };
        let mut response = response?;
        if response.mime == "text/css" {
            response.body = ::fixtures::stylesheet(&response.body);
        }
        Ok(response)
    }
}

#[test]
fn decoding() {
    for (path, bytes) in [
        ("/site/index.html", vec![0xed, 0xa0, 0x80]),
        ("/site/style.css", vec![0xf0, 0x9f]),
        ("/site/main.js", vec![0xf4, 0x90, 0x80, 0x80]),
    ] {
        let mut page = Page::new(
            "app://demo/site/index.html",
            Extent {
                width: 200,
                height: 120,
            },
        )
        .unwrap();
        let failure = page
            .load(&mut Invalid {
                source: source(),
                path,
                bytes,
            })
            .unwrap_err();
        assert!(failure.0.contains("utf-8"), "{failure}");
        assert_eq!(page.inspect().state, State::Failed);
        assert!(page.frame().is_none());
    }
}
