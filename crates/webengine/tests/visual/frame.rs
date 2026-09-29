use graphics::shared::{Backend, Cpu, Extent};
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
        Ok(Response { status: 200, mime: "text/html".into(), body: b"<head><script src='./main.js'></script></head><body id='body'><button id='a' style='height: 32px; background-color: #112233;'>A</button><button id='b' style='height: 32px; background-color: #445566;'>fiB</button><button id='c' style='height: 32px; background-color: #778899;'>C</button></body>".to_vec() })
    }
}
fn page() -> Page {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut Source).unwrap();
    page.evaluate("const body = document.getElementById('body'); const a = document.getElementById('a'); const b = document.getElementById('b'); const c = document.getElementById('c'); a.addEventListener('click', function (event) { bridge.postMessage('a'); }); b.addEventListener('click', function (event) { bridge.postMessage('b'); });").unwrap();
    page
}

#[test]
fn composition() {
    let mut page = page();
    let first = page.frame().unwrap();
    page.acknowledge(first.id()).unwrap();
    let before = page.inspect();
    let retained = page.hit(8, 8);
    page.scroll(32).unwrap();
    let next = page.frame().unwrap();
    assert_eq!(first.scene().id(), next.scene().id());
    assert_ne!(first.id(), next.id());
    assert_ne!(first.id(), first.scene().id());
    assert_eq!(before.painted, page.inspect().painted);
    assert_eq!(before.revision, page.inspect().revision);
    assert_eq!(page.inspect().shown.unwrap().offset(0), 0);
    assert_eq!(Cpu.render(&first).unwrap().pixels[20 * 64 + 60], 0x112233);
    assert_eq!(Cpu.render(&next).unwrap().pixels[20 * 64 + 60], 0x445566);
    page.acknowledge(next.id()).unwrap();
    page.dispatch(&retained).unwrap();
    assert_eq!(page.messages(), ["a"]);
    page.dispatch(&page.hit(8, 8)).unwrap();
    assert_eq!(page.messages(), ["b"]);
    assert_eq!(page.inspect().shown.unwrap().offset(0), 32);
    let image = Cpu.render(&next).unwrap();
    page.close();
    assert_eq!(Cpu.render(&next).unwrap(), image);
    assert!(page.scroll(1).is_err());
}

#[test]
fn convergence() {
    let mut page = page();
    let first = page.frame().unwrap();
    page.acknowledge(first.id()).unwrap();
    page.scroll(i64::MAX).unwrap();
    let bottom = page.frame().unwrap();
    assert_eq!(bottom.visual().offset(0), 64);
    page.evaluate("body.removeChild(c);").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().visual.offset(0), 32);
    assert!(page.frame().is_none());
    page.acknowledge(bottom.id()).unwrap();
    assert_eq!(page.inspect().shown.unwrap().offset(0), 64);
    let next = page.frame().unwrap();
    assert_ne!(bottom.scene().id(), next.scene().id());
    page.acknowledge(next.id()).unwrap();
    assert_eq!(page.inspect().shown.unwrap().offset(0), 32);
    page.scroll(i64::MIN).unwrap();
    let top = page.frame().unwrap();
    assert_eq!(top.visual().offset(0), 0);
    page.acknowledge(top.id()).unwrap();
    page.scroll(-1).unwrap();
    assert!(page.frame().is_none());
}

#[test]
fn coalescing() {
    let mut page = page();
    let first = page.frame().unwrap();
    page.scroll(16).unwrap();
    page.scroll(16).unwrap();
    page.scroll(16).unwrap();
    let latest = page.inspect();
    assert_eq!(latest.visual.offset(0), 48);
    assert_eq!(latest.visual.revision, 3);
    assert!(page.frame().is_none());
    page.acknowledge(first.id()).unwrap();
    let next = page.frame().unwrap();
    assert_eq!(next.id(), latest.pending.unwrap());
    assert_eq!(next.scene().id(), first.scene().id());
    page.complete(next.id(), Err(Fault("output failed".into())))
        .unwrap();
    assert!(page.scroll(1).is_err());
    assert_eq!(page.inspect().shown.unwrap().offset(0), 0);
}
