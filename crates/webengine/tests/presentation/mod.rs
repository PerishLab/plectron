mod pipeline;
mod wrappers;
use graphics::shared::Extent;
use std::sync::Arc;
use webengine::{
    Fault, Page,
    resource::{Provider, Response},
};

struct Source;
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document, bridge", "").into_bytes(),
            });
        }
        Ok(Response { status: 200, mime: "text/html".into(), body: b"<head><script src='./main.js'></script></head><body id='body'><button id='a' style='height: 40px;'>A</button><button id='b' style='height: 40px;'>B</button></body>".to_vec() })
    }
}
fn page() -> Page {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 200,
            height: 200,
        },
    )
    .unwrap();
    page.load(&mut Source).unwrap();
    page.evaluate("const a = document.getElementById('a'); const b = document.getElementById('b'); a.addEventListener('click', function (event) { bridge.postMessage('a'); }); b.addEventListener('click', function (event) { bridge.postMessage('b'); });").unwrap();
    page
}

#[test]
fn publication() {
    let mut page = page();
    let before = page.inspect();
    let repeated = page.inspect();
    assert!(Arc::ptr_eq(&before.nodes, &repeated.nodes));
    assert!(Arc::ptr_eq(&before.fragments, &repeated.fragments));
    page.evaluate("const body = document.getElementById('body'); a.setText('changed'); a.setAttribute('class', 'active'); body.removeChild(b);").unwrap();
    let pending = page.inspect();
    assert!(!Arc::ptr_eq(&before.nodes, &pending.nodes));
    assert!(Arc::ptr_eq(&before.fragments, &pending.fragments));
    page.render().unwrap();
    let changed = page.inspect();
    assert!(Arc::ptr_eq(&pending.nodes, &changed.nodes));
    assert!(!Arc::ptr_eq(&before.fragments, &changed.fragments));
    assert!(Arc::ptr_eq(&before.nodes[0], &changed.nodes[0]));
    let restored = webengine::wire::Bundle::new(changed.clone())
        .restore()
        .unwrap();
    assert_eq!(restored.fragments, changed.fragments);
    assert!(changed.nodes.iter().any(|node| node.content == "changed"));
    assert!(!changed.nodes.iter().any(|node| node.content == "B"));
    assert!(before.nodes.iter().any(|node| node.content == "B"));
    assert!(changed.nodes.iter().any(|node| {
        node.attributes
            .get("class")
            .is_some_and(|value| value == "active")
    }));
    page.evaluate("body.appendChild(b); b.appendChild(a);")
        .unwrap();
    let moved = page.inspect();
    let a = moved
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "a"))
        .unwrap();
    let b = moved
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "b"))
        .unwrap();
    assert_eq!(a.parent, Some(b.id));
    for index in 0..1100 {
        page.evaluate(&format!("a.setAttribute('class', 'class{index}');"))
            .unwrap();
    }
    let rebuilt = page.inspect();
    assert!(rebuilt.nodes.iter().any(|node| {
        node.attributes
            .get("class")
            .is_some_and(|value| value == "class1099")
    }));
    assert!(moved.nodes.iter().any(|node| {
        node.attributes
            .get("class")
            .is_some_and(|value| value == "active")
    }));
    page.close();
    assert!(page.inspect().nodes.is_empty());
    assert!(rebuilt.nodes.iter().any(|node| node.content == "changed"));
    assert!(!before.fragments.is_empty());
    assert_eq!(restored.fragments, changed.fragments);
}
fn present(page: &mut Page) -> u64 {
    let scene = page.frame().unwrap();
    page.acknowledge(scene.id()).unwrap();
    scene.id()
}

#[test]
fn identity() {
    let mut first = page();
    let mut second = page();
    let a = first.frame().unwrap();
    let b = second.frame().unwrap();
    assert_eq!(a.scene().revision(), b.scene().revision());
    assert_ne!(a.id(), b.id());
    assert_eq!(a.id(), a.clone().id());
    assert!(first.acknowledge(b.id()).is_err());
    first.evaluate("a.setText('changed');").unwrap();
    first.render().unwrap();
    assert!(first.frame().is_none());
    assert!(first.acknowledge(a.id()).unwrap());
    assert!(first.acknowledge(a.id()).is_err());
    assert_eq!(first.presented(), Some(a.scene().revision()));
    let latest = first.frame().unwrap();
    first.close();
    assert!(first.acknowledge(latest.id()).is_err());
}

#[test]
fn targeting() {
    let mut page = page();
    let unshown = page.hit(8, 48);
    present(&mut page);
    page.dispatch(&unshown).unwrap();
    assert!(page.messages().is_empty());
    let retained = page.hit(8, 48);
    page.evaluate("a.setAttribute('style', 'height: 120px;'); b.setText('Replaced');")
        .unwrap();
    page.render().unwrap();
    let old = page.hit(8, 48);
    page.dispatch(&old).unwrap();
    assert_eq!(page.messages(), ["b"]);
    present(&mut page);
    page.dispatch(&retained).unwrap();
    assert_eq!(page.messages(), ["b"]);
    let current = page.hit(8, 48);
    page.dispatch(&current).unwrap();
    assert_eq!(page.messages(), ["a"]);
    page.evaluate("document.getElementById('body').removeChild(b);")
        .unwrap();
    page.dispatch(&retained).unwrap();
    assert!(page.messages().is_empty());
}

#[test]
fn failures() {
    let mut page = page();
    let initial = present(&mut page);
    page.evaluate("a.setAttribute('style', 'height: 120px;');")
        .unwrap();
    page.render().unwrap();
    let next = page.frame().unwrap();
    assert!(
        !page
            .complete(next.id(), Err(Fault("output lost".into())))
            .unwrap()
    );
    let snapshot = page.inspect();
    assert_eq!(snapshot.frame, Some(initial));
    assert_eq!(snapshot.output.as_deref(), Some("output lost"));
    assert!(page.acknowledge(next.id()).is_err());
    page.evaluate("a.setText('Inspectable');").unwrap();
    page.render().unwrap();
    assert!(page.frame().is_none());
    let hit = page.hit(8, 48);
    page.dispatch(&hit).unwrap();
    assert_eq!(page.messages(), ["b"]);
}

#[test]
fn foreign() {
    let mut first = page();
    let mut second = page();
    present(&mut first);
    present(&mut second);
    assert!(second.dispatch(&first.hit(8, 48)).is_err());
    assert!(second.messages().is_empty());
}

#[test]
fn timing() {
    let mut page = page();
    page.evaluate("a.setText('Changed');").unwrap();
    page.render().unwrap();
    let timing = page.timing();
    assert!(timing.content >= timing.scene);
    assert_eq!(timing.collections, 0);
    page.inspect();
    assert_eq!(page.timing(), timing);
    page.render().unwrap();
    assert_eq!(page.timing().scene, 0);
    page.close();
    assert!(page.render().is_err());
    assert_eq!(page.timing().snapshot, 0);
    assert_eq!(page.timing().install, 0);
}

#[test]
fn collection() {
    let mut page = page();
    for _ in 0..80 {
        page.evaluate("document.createElement('div');").unwrap();
    }
    page.render().unwrap();
    let timing = page.timing();
    assert_eq!(timing.collections, 1);
    assert!(timing.content >= timing.checkpoint.saturating_add(timing.collection));
    page.render().unwrap();
    assert_eq!(page.timing().collections, 0);
    assert_eq!(page.timing().collection, 0);
}

#[test]
fn milestones() {
    let mut page = page();
    let initial = page.inspect().milestones;
    assert!(initial.resources <= initial.fonts);
    assert!(initial.fonts <= initial.pictures);
    assert!(initial.pictures <= initial.loaded);
    assert!(initial.loaded <= initial.runtime);
    assert!(initial.runtime <= initial.activation);
    assert!(initial.activation <= initial.scene);
    assert!(initial.scene <= initial.publication);
    assert_eq!((initial.frame, initial.presentation), (None, None));
    let frame = page.frame().unwrap();
    let issued = page.inspect().milestones;
    assert!(initial.publication <= issued.frame);
    assert_eq!(issued.presentation, None);
    assert!(page.acknowledge(frame.id()).unwrap());
    let presented = page.inspect().milestones;
    assert!(issued.frame <= presented.presentation);
    page.render().unwrap();
    assert_eq!(page.inspect().milestones, presented);
}
mod isolation;
