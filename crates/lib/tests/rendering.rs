use domengine::Extent;
use jsruntime::Script;
use project::{Adapters, Linker, Memory, Module, Resolver, Target};
use webengine::resource::{Provider, Request, Response, Url};
use webengine::{Fault, Page, State};

struct Source {
    script: Option<Script>,
    body: Vec<u8>,
    mime: String,
    status: u16,
    requests: Vec<String>,
}

fn link(entry: &str, dependency: &str) -> Result<Script, project::Diagnostic> {
    let (package, engine) = fixtures::engine();
    let module = Module::new("app/main.ts").unwrap();
    Linker::new(
        Memory::new([
            (module.clone(), entry.into()),
            (Module::new("app/failure.ts").unwrap(), dependency.into()),
            engine,
        ]),
        Resolver::new([package]).unwrap(),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(module, Target::Script)
    .map(plectron::runtime::source::script)
}

impl Source {
    fn new(script: Script) -> Self {
        Self {
            body: script.source().text().as_bytes().to_vec(),
            script: Some(script),
            mime: "text/javascript".into(),
            status: 200,
            requests: Vec::new(),
        }
    }

    fn response(&mut self, url: &Url) -> Response {
        self.requests.push(url.to_string());
        if url.path().ends_with("index.html") {
            Response {
                status: 200,
                mime: "text/html".into(),
                body: b"<html><head><script src='./main.ts'></script></head><body id='body'></body></html>".to_vec(),
            }
        } else {
            Response {
                status: self.status,
                mime: self.mime.clone(),
                body: self.body.clone(),
            }
        }
    }
}

impl Provider for Source {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        Ok(self.response(&request.url))
    }

    fn script(&mut self, url: &Url) -> Result<webengine::resource::Script, Fault> {
        Ok(webengine::resource::Script {
            response: self.response(url),
            source: self.script.take(),
        })
    }
}

fn page() -> Page {
    Page::new(
        "app://test/app/index.html",
        Extent {
            width: 200,
            height: 120,
        },
    )
    .unwrap()
}

fn expected(text: &str) -> String {
    let start = text.find("throw").unwrap();
    let end = start + text[start..].find(';').unwrap() + 1;
    format!("app/failure.ts:{start}..{end}:")
}

#[test]
fn synchronous() {
    let text = "export const fail = function (): never { throw 'source'; };";
    let script = link("import {fail} from './failure.ts'; fail();", text).unwrap();
    let mut source = Source::new(script);
    let mut page = page();
    let failure = page.load(&mut source).unwrap_err();
    assert!(failure.0.starts_with(&expected(text)), "{failure}");
    assert_eq!(page.inspect().state, State::Failed);
    assert_eq!(source.requests.len(), 2);
    assert!(source.script.is_none());
}

#[test]
fn callbacks() {
    let text = "export const fail = function (): never { throw 'listener'; };";
    let script = link("import {document, bridge} from '@plectron/engine'; import {fail} from './failure.ts'; const body = document.getElementById('body'); body.addEventListener('test', fail); body.addEventListener('test', () => bridge.postMessage('continued')); bridge.onMessage(() => body.dispatchEvent('test', ''));", text).unwrap();
    let mut source = Source::new(script);
    let mut page = page();
    page.load(&mut source).unwrap();
    drop(source);
    page.evaluate("const later = 1;").unwrap();
    page.collect().unwrap();
    page.receive("dispatch".into()).unwrap();
    page.render().unwrap();
    let snapshot = page.inspect();
    let record = snapshot.events.last().unwrap();
    assert_eq!(record.errors, 1);
    assert!(
        record
            .exception
            .as_ref()
            .unwrap()
            .starts_with(&expected(text))
    );
    assert!(
        record.deliveries[0]
            .error
            .as_ref()
            .unwrap()
            .starts_with(&expected(text))
    );
    assert!(record.deliveries[1].error.is_none());
    assert_eq!(page.messages(), ["continued"]);
}

#[test]
fn asynchronous() {
    let text = "export const fail = async function (): Promise<never> { await 1; throw 'late'; };";
    let script = link("import {bridge} from '@plectron/engine'; import {fail} from './failure.ts'; bridge.onMessage(() => { fail().then(); });", text).unwrap();
    let mut source = Source::new(script);
    let mut page = page();
    page.load(&mut source).unwrap();
    drop(source);
    page.collect().unwrap();
    page.receive("start".into()).unwrap();
    page.render().unwrap();
    assert!(page.inspect().jobs.rejected[0].starts_with(&expected(text)));
}

#[test]
fn mismatch() {
    let script = link(
        "import {bridge} from '@plectron/engine'; bridge.postMessage('effect');",
        "",
    )
    .unwrap();
    let mut source = Source::new(script);
    source.body = b"throw 'changed';".to_vec();
    let mut page = page();
    assert!(
        page.load(&mut source)
            .unwrap_err()
            .0
            .contains("source metadata does not match body")
    );
    assert!(page.messages().is_empty());
    assert_eq!(page.inspect().state, State::Failed);
}

#[test]
fn visual() {
    let text = "export const fail = function (): never { throw 'observer'; };";
    let script = link("import {document, visual, bridge} from '@plectron/engine'; import {fail} from './failure.ts'; const host = document.createElement('div'); host.setAttribute('style', 'width:64px;height:64px;overflow:auto;'); const child = document.createElement('div'); child.setAttribute('style', 'height:200px;'); host.appendChild(child); document.getElementById('body').appendChild(host); bridge.onMessage(() => { visual.observe(host, fail); child.setAttribute('style', 'height:201px;'); });", text).unwrap();
    let mut source = Source::new(script);
    let mut page = page();
    page.load(&mut source).unwrap();
    drop(source);
    page.collect().unwrap();
    page.receive("observe".into()).unwrap();
    let failure = page.render().unwrap_err();
    assert!(failure.0.starts_with(&expected(text)), "{failure}");
}

#[test]
fn admission() {
    let script = link(
        "import {bridge} from '@plectron/engine'; bridge.postMessage('effect');",
        "",
    )
    .unwrap();
    for (status, mime, oversized) in [
        (404, "text/javascript", false),
        (200, "text/plain", false),
        (200, "text/javascript", true),
    ] {
        let mut source = Source::new(script.clone());
        source.status = status;
        source.mime = mime.into();
        if oversized {
            source.body.resize(65_537, b' ');
        }
        let mut page = page();
        assert!(page.load(&mut source).is_err());
        assert!(page.messages().is_empty());
    }
}

#[test]
fn anonymous() {
    struct Bare(Source);
    impl Provider for Bare {
        fn request(&mut self, request: &Request) -> Result<Response, Fault> {
            self.0.request(request)
        }
    }
    let script = link("throw 'unmapped';", "").unwrap();
    let mut source = Bare(Source::new(script));
    let failure = page().load(&mut source).unwrap_err();
    assert!(!failure.0.contains("app/main.ts:"));
    assert!(failure.0.contains("unmapped"));
    assert_eq!(source.0.requests.len(), 2);
}
