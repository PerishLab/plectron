mod fonts;
use domengine::Extent;
use std::collections::BTreeMap;
use webengine::resource::{Provider, Response};
use webengine::{Fault, Page, State};
mod editing;
mod events;
mod keyboard;
mod performance;
mod text;

struct File {
    mime: String,
    body: String,
}

struct Source {
    files: BTreeMap<String, File>,
    requested: Vec<String>,
}

mod provider;

fn source() -> Source {
    Source {
        requested: Vec::new(),
        files: [
            (
                "/site/index.html".into(),
                (
                    "text/html",
                    "<html><head><link rel='stylesheet' href='./style.css'><script src='./main.js'></script></head><body id='body'><button id='button'>Count</button></body></html>",
                ),
            ),
            (
                "/site/style.css".into(),
                (
                    "text/css",
                    "body { padding: 10px; } button { height: 40px; background-color: #225394; }",
                ),
            ),
            (
                "/site/main.js".into(),
                (
                    "text/javascript",
                    "let count = 0; const button = document.getElementById('button'); button.addEventListener('click', function (event) { count = count + 1; button.setText('Count: ' + count); bridge.postMessage('count:' + count); }); document.getElementById('body').addEventListener('click', function (event) { bridge.postMessage('bubble'); }); bridge.onMessage(function (text) { button.setText(text); });",
                ),
            ),
        ].into_iter().map(|(path, (mime, body))| (path, File { mime: mime.into(), body: body.into() })).collect(),
    }
}

fn loaded(source: &mut Source) -> Page {
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 120,
        },
    )
    .unwrap();
    page.load(source).unwrap();
    assert_eq!(page.inspect().state, State::Ready);
    assert_eq!(page.evaluate("count;").unwrap(), "0");
    let scene = page.frame().unwrap();
    assert_eq!(
        graphics::shared::Backend::render(&mut graphics::shared::Cpu, &scene)
            .unwrap()
            .pixels
            .len(),
        24_000
    );
    assert!(page.acknowledge(scene.id()).unwrap());
    page
}

#[test]
fn pipeline() {
    let mut source = source();
    let mut page = loaded(&mut source);
    assert_eq!(
        source.requested,
        [
            "app://demo/site/index.html",
            "app://demo/site/style.css",
            "app://demo/site/main.js"
        ]
    );
    assert_eq!(page.evaluate("count;").unwrap(), "0");
    let before = page.inspect().painted;
    page.click(50, 30).unwrap();
    assert_eq!(page.evaluate("count;").unwrap(), "1");
    assert_eq!(page.messages(), ["count:1", "bubble"]);
    assert_eq!(page.inspect().painted, before);
    page.render().unwrap();
    let snapshot = page.inspect();
    assert!(snapshot.revision > before);
    assert_eq!(snapshot.revision, snapshot.painted);
    assert!(snapshot.nodes.iter().any(|node| node.content == "Count: 1"));
    let scene = page.frame().unwrap();
    assert!(!page.acknowledge(scene.id()).unwrap());
    page.render().unwrap();
    assert!(page.frame().is_none());
    page.receive("reset".into()).unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "reset")
    );
}

#[test]
fn isolation() {
    let mut first = loaded(&mut source());
    let mut second = loaded(&mut source());
    first.evaluate("count = 100;").unwrap();
    assert_eq!(second.evaluate("count;").unwrap(), "0");
    for code in [
        "window;",
        "location;",
        "history;",
        "navigator;",
        "platform;",
        "document.write('<p>injected</p>');",
    ] {
        assert!(first.evaluate(code).is_err(), "{code}");
    }
    assert_eq!(
        first
            .evaluate("document.getElementById('button') === button;")
            .unwrap(),
        "true"
    );
    first
        .evaluate("let rejected = false; fetch('/missing').catch(() => { rejected = true; });")
        .unwrap();
    first.poll().unwrap();
    assert_eq!(first.evaluate("rejected;").unwrap(), "true");
    assert!(second.evaluate("rejected;").is_err());
    first.close();
    assert_eq!(first.inspect().state, State::Closed);
    assert!(first.inspect().nodes.is_empty());
    assert!(first.click(50, 30).is_err());
    assert!(first.receive("late".into()).is_err());
    assert!(first.evaluate("1;").is_err());
    assert!(first.frame().is_none());
    assert!(first.load(&mut source()).is_err());
}

#[test]
fn propagation() {
    let mut page = loaded(&mut source());
    page.evaluate(
        "button.addEventListener('click', function (event, context) { context.stopPropagation(); });",
    )
    .unwrap();
    page.click(30, 20).unwrap();
    assert_eq!(page.messages(), ["count:1"]);
    page.evaluate("document.getElementById('body').removeChild(button);")
        .unwrap();
    page.click(30, 20).unwrap();
    assert!(page.messages().is_empty());
}

#[test]
fn failure() {
    let mut source = source();
    source.files.remove("/site/main.js");
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 100,
        },
    )
    .unwrap();
    assert!(page.load(&mut source).is_err());
    let snapshot = page.inspect();
    assert_eq!(snapshot.state, State::Failed);
    assert!(snapshot.fault.is_some());
    assert!(page.frame().is_none());
    assert!(page.acknowledge(0).is_err());
    page.close();
    assert_eq!(page.inspect().state, State::Closed);
}

#[test]
fn retained() {
    let mut page = loaded(&mut source());
    page.evaluate("let previous = null; const listener = function (event, context) { if (previous !== null) { previous.stopPropagation(); } previous = context; }; button.addEventListener('click', listener); button.addEventListener('click', listener);").unwrap();
    page.click(80, 30).unwrap();
    assert_eq!(page.messages(), ["count:1", "bubble"]);
    page.render().unwrap();
    let scene = page.frame().unwrap();
    page.acknowledge(scene.id()).unwrap();
    page.click(80, 30).unwrap();
    assert_eq!(page.messages(), ["count:2", "bubble"]);
}

#[test]
fn replay() {
    let mut page = Page::new(
        "app://demo/site/index.html",
        Extent {
            width: 200,
            height: 120,
        },
    )
    .unwrap();
    page.load(&mut source()).unwrap();
    assert_eq!(page.state(), State::Ready);
    assert_eq!(page.presented(), None);
    let old = page.frame().unwrap();
    let pixels = graphics::shared::Backend::render(&mut graphics::shared::Cpu, &old).unwrap();
    page.click(50, 30).unwrap();
    page.render().unwrap();
    assert!(page.frame().is_none());
    assert!(page.acknowledge(old.id()).unwrap());
    let new = page.frame().unwrap();
    assert!(!page.acknowledge(new.id()).unwrap());
    assert!(page.acknowledge(new.id()).is_err());
    assert_ne!(
        graphics::shared::Backend::render(&mut graphics::shared::Cpu, &new).unwrap(),
        pixels
    );
    page.close();
    assert_eq!(
        graphics::shared::Backend::render(&mut graphics::shared::Cpu, &old).unwrap(),
        pixels
    );
}

#[test]
fn exceptions() {
    let mut page = loaded(&mut source());
    page.evaluate("try { throw Error('caught'); } catch (error) { button.setText(error.message); } finally { bridge.postMessage('finished'); }").unwrap();
    page.render().unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "caught")
    );
    assert_eq!(page.messages(), ["finished"]);
    let failure = page
        .evaluate("throw TypeError('content failure');")
        .unwrap_err();
    assert!(failure.0.contains("TypeError: content failure"));
    assert_eq!(page.inspect().state, State::Ready);
    assert!(page.evaluate("try { bridge.postMessage(1); } catch { bridge.postMessage('caught'); } finally { bridge.postMessage('finished'); }").is_err());
    assert!(page.messages().is_empty());
    page.evaluate("bridge.postMessage('resumed');").unwrap();
    assert_eq!(page.messages(), ["resumed"]);
}

#[test]
fn classes() {
    let mut page = loaded(&mut source());
    page.evaluate("class Count { storage = 0; get count() { return this.storage; } set count(n) { this.storage = n; } increment(step = 1) { this.count += step; } } class Counter extends Count { constructor(node = button) { super(); this.node = node; node.addEventListener('click', (event, context, ...extra) => { this.extra = extra.length; this.increment(); }); } increment() { super.increment(); this.node.setText('Class:' + this.count); bridge.postMessage('class:' + this.count); } } const counter = new Counter();").unwrap();
    page.click(50, 30).unwrap();
    page.render().unwrap();
    assert_eq!(page.evaluate("counter.count;").unwrap(), "1");
    assert_eq!(page.evaluate("counter.extra;").unwrap(), "0");
    assert_eq!(page.messages(), ["count:1", "class:1", "bubble"]);
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Class:1")
    );
    let frame = page.frame().unwrap();
    let image = graphics::shared::Backend::render(&mut graphics::shared::Cpu, &frame).unwrap();
    assert_eq!(image.pixels[30 * 200 + 150], 0x225394);
    let mut other = loaded(&mut source());
    assert!(other.evaluate("counter;").is_err());
    assert!(other.evaluate("Counter;").is_err());
}

mod pictures;
