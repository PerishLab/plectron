use graphics::shared::{Backend, Cpu, Extent};
use webengine::{Disposition, Fault, Page, Snapshot, change, resource::Provider};
#[path = "presentation/scrolling.rs"]
mod scrolling;

#[derive(Clone)]
struct Source(fixtures::Application);

impl Source {
    fn new() -> Self {
        Self(fixtures::Application::new("spaces"))
    }
}

impl Provider for Source {
    fn request(
        &mut self,
        request: &webengine::resource::Request,
    ) -> Result<webengine::resource::Response, Fault> {
        let resource = self.0.load(request.url.path()).map_err(Fault)?;
        request.accept(webengine::resource::Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
}
fn loaded() -> Page {
    let mut page = Page::new(
        "app://spaces/index.html",
        Extent {
            width: 360,
            height: 240,
        },
    )
    .unwrap();
    page.load(&mut Source::new()).unwrap();
    page
}
fn setup() -> Page {
    let mut page = loaded();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    page
}
fn space(snapshot: &Snapshot, name: &str) -> u64 {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == name))
        .unwrap();
    let boxspace = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .space;
    snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(boxspace) && mapping.range.is_some())
        .unwrap()
        .id
}
fn accept(page: &mut Page) -> graphics::shared::Frame {
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    frame
}
#[test]
fn nested() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    let outer = space(&initial, "outer");
    let old = page.hit(40, 130);
    assert_eq!(old.frame(), initial.frame.unwrap());
    assert_eq!(old.scene(), initial.displayed.unwrap());
    assert_eq!(page.wheel(40, 130, 40).unwrap(), Disposition::Applied);
    let pending = page.inspect();
    assert_eq!(pending.scene, initial.scene);
    assert_eq!(pending.painted, initial.painted);
    assert_eq!(pending.revision, initial.revision);
    assert_eq!(pending.visual.offset(inner), 40);
    assert_eq!(pending.visual.offset(outer), 0);
    assert_eq!(pending.shown.unwrap().offset(inner), 0);
    let frame = accept(&mut page);
    page.dispatch(&old).unwrap();
    assert_eq!(page.evaluate("last;").unwrap(), "a");
    page.dispatch(&page.hit(40, 130)).unwrap();
    assert_eq!(page.evaluate("last;").unwrap(), "b");
    page.pan(frame.scene().id(), outer, 24).unwrap();
    let next = accept(&mut page);
    assert_eq!(next.visual().offset(inner), 40);
    assert_eq!(next.visual().offset(outer), 24);
    assert_eq!(next.scene().id(), frame.scene().id());
    let before = Cpu.render(&next).unwrap();
    page.close();
    assert_eq!(Cpu.render(&next).unwrap(), before);
}
#[test]
fn stale() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.evaluate("document.getElementById('c').setText('changed');")
        .unwrap();
    page.render().unwrap();
    let changed = page.inspect();
    assert_ne!(changed.scene, initial.scene);
    assert_eq!(changed.displayed, initial.scene);
    assert_eq!(page.wheel(40, 130, 8).unwrap(), Disposition::Applied);
    assert_eq!(page.inspect().visual.offset(inner), 8);
}
#[test]
fn obsolete() {
    let mut page = setup();
    let initial = page.inspect();
    let old = space(&initial, "inner");
    page.evaluate(
        "const body = document.getElementById('body'); const outer = document.getElementById('outer'); body.removeChild(outer); const replacement = document.createElement('div'); replacement.setAttribute('id', 'outer'); const inner = document.createElement('div'); inner.setAttribute('id', 'inner'); for (const id of ['x', 'y', 'z']) { const button = document.createElement('button'); button.setAttribute('id', id); button.setText(id); inner.appendChild(button); } replacement.appendChild(inner); body.appendChild(replacement);",
    )
    .unwrap();
    page.render().unwrap();
    let current = page.inspect();
    let replacement = space(&current, "inner");
    assert_ne!(replacement, old);
    assert_eq!(current.displayed, initial.displayed);
    assert_eq!(page.wheel(40, 130, 8).unwrap(), Disposition::Obsolete);
    let after = page.inspect();
    assert_eq!(after.visual.offset(replacement), 0);
    assert!(!after.visual.offsets.contains_key(&old));
    assert!(page.pan(initial.scene.unwrap(), old, 8).is_err());
}

#[test]
fn outcomes() {
    let mut page = loaded();
    assert_eq!(page.wheel(40, 130, 8).unwrap(), Disposition::Deferred);
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    assert_eq!(page.wheel(400, 300, 8).unwrap(), Disposition::Blocked);
}
#[test]
fn clipping() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    page.pan(initial.scene.unwrap(), inner, 56).unwrap();
    accept(&mut page);
    page.dispatch(&page.hit(40, 110)).unwrap();
    assert_eq!(page.evaluate("last;").unwrap(), "before");
    page.dispatch(&page.hit(40, 150)).unwrap();
    assert_eq!(page.evaluate("last;").unwrap(), "c");
    let revision = page.inspect().visual.revision;
    assert_eq!(page.wheel(40, 150, 1000).unwrap(), Disposition::Blocked);
    assert_eq!(page.inspect().visual.revision, revision);
    assert!(page.frame().is_none());
}

#[test]
fn occlusion() {
    let mut page = setup();
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    let outer = space(&initial, "outer");
    page.evaluate(
        "document.getElementById('effect').setAttribute('style', 'translate: 8px -120px;');",
    )
    .unwrap();
    page.render().unwrap();
    accept(&mut page);
    page.wheel(40, 90, 12).unwrap();
    let frame = accept(&mut page);
    assert_eq!(frame.visual().offset(0), 12);
    assert_eq!(frame.visual().offset(inner), 0);
    assert_eq!(frame.visual().offset(outer), 0);
}

#[test]
fn gutter() {
    let mut page = setup();
    page.evaluate("document.getElementById('a').setAttribute('style', 'margin: 12px;');")
        .unwrap();
    page.render().unwrap();
    accept(&mut page);
    let initial = page.inspect();
    let inner = space(&initial, "inner");
    let outer = space(&initial, "outer");
    let node = initial
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "inner"))
        .unwrap();
    let region = initial
        .visible
        .iter()
        .find(|region| region.node == Some(node.id))
        .unwrap();
    page.wheel(region.rect.x + 2, region.rect.y + 2, 12)
        .unwrap();
    let after = page.inspect();
    assert_eq!(after.visual.offset(inner), 12);
    assert_eq!(after.visual.offset(outer), 0);
    assert_eq!(after.scene, initial.scene);
    assert_eq!(after.revision, initial.revision);
    assert_eq!(after.shown.unwrap().offset(inner), 0);
    accept(&mut page);
    assert_eq!(page.inspect().shown.unwrap().offset(inner), 12);
}
