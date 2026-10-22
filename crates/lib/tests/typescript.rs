use webengine::resource::{Provider, Request, Response, Url};
use webengine::{Fault, Page};

struct Application(fixtures::Application);
impl Provider for Application {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        let resource = self.0.load(request.url.path()).map_err(Fault)?;
        Ok(Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
    fn script(&mut self, url: &Url) -> Result<webengine::resource::Script, Fault> {
        let resource = self.0.load(url.path()).map_err(Fault)?;
        Ok(webengine::resource::Script {
            response: Response {
                status: 200,
                mime: resource.mime,
                body: resource.body,
            },
            source: resource.script.map(plectron::runtime::source::script),
        })
    }
}

fn page() -> Page {
    let mut page = Page::new(
        "app://test/index.html",
        domengine::Extent {
            width: 640,
            height: 400,
        },
    )
    .unwrap();
    page.load(&mut Application(fixtures::Application::new("typescript")))
        .unwrap();
    page
}

fn prefix(text: &str) -> String {
    let model = std::fs::read_to_string(fixtures::root().join("typescript/src/model.ts")).unwrap();
    let start = model.find(text).unwrap();
    format!(
        "apps/typescript/src/model.ts:{start}..{}:",
        start + text.len()
    )
}

#[test]
fn application() {
    let mut page = page();
    page.collect().unwrap();
    for count in 1..=3 {
        page.receive("increment".into()).unwrap();
        page.render().unwrap();
        assert_eq!(page.messages(), [format!("count:{count}")]);
        assert!(
            page.inspect()
                .nodes
                .iter()
                .any(|node| node.content == format!("Count: {count}"))
        );
    }
    page.receive("dispose".into()).unwrap();
    page.render().unwrap();
    page.collect().unwrap();
    page.receive("increment".into()).unwrap();
    assert!(page.messages().is_empty());
    assert!(
        !page
            .inspect()
            .nodes
            .iter()
            .any(|node| node.content.starts_with("Count:"))
    );
    page.close();
    assert!(page.evaluate("0;").is_err());
}

#[test]
fn diagnostics() {
    let mut page = page();
    page.collect().unwrap();
    page.receive("failure".into()).unwrap();
    page.render().unwrap();
    let snapshot = page.inspect();
    let record = snapshot.events.last().unwrap();
    assert_eq!(record.errors, 1);
    assert!(
        record
            .exception
            .as_ref()
            .unwrap()
            .starts_with(&prefix("throw \"typescript listener\";"))
    );
    page.receive("asynchronous".into()).unwrap();
    page.render().unwrap();
    assert!(page.inspect().jobs.rejected[0].starts_with(&prefix("throw \"typescript async\";")));
    page.receive("increment".into()).unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["count:1"]);
}

#[test]
fn provenance() {
    let application = fixtures::Application::new("typescript");
    let resource = application.load("/main.ts").unwrap();
    assert_eq!(resource.mime, "text/javascript");
    let script = resource.script.unwrap();
    assert_eq!(resource.body, script.javascript.as_bytes());
    assert!(
        script
            .origins
            .iter()
            .any(|origin| origin.module.path() == "apps/typescript/src/model.ts")
    );
    assert!(
        script
            .origins
            .iter()
            .any(|origin| origin.module.path() == "apps/typescript/src/view.pt")
    );
    let resource = application.load("/style.css").unwrap();
    assert!(resource.script.is_none());
    assert!(application.load("/../Cargo.toml").is_err());
}
