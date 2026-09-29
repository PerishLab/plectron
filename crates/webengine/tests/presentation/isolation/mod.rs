use graphics::shared::{Backend, Cpu, Extent};
mod estimates;
mod worker;
use webengine::{
    Fault, Page,
    resource::{Provider, Response},
};

struct Source(String);
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document, scroll", "").into_bytes(),
            });
        }
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: format!("<head><script src='./main.js'></script></head>{}", self.0).into_bytes(),
        })
    }
}
fn source(isolated: bool) -> Source {
    let containment = if isolated {
        "contain: size layout paint;"
    } else {
        ""
    };
    let mut html = String::from("<body id='body'>");
    for index in 0..20 {
        html.push_str(&format!("<div id='row{index}' style='height:32px; {containment}'><div id='child{index}' style='height:12px; background-color:#123456;'>row {index}</div></div>"));
    }
    html.push_str("</body>");
    Source(html)
}
fn page(isolated: bool) -> Page {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut source(isolated)).unwrap();
    page
}
fn image(page: &mut Page) -> graphics::shared::Image {
    let frame = page.frame().unwrap();
    let image = Cpu.render(&frame).unwrap();
    page.acknowledge(frame.id()).unwrap();
    image
}
fn scroller(page: &mut Page, id: &str) -> (u64, usize) {
    let snapshot = page.inspect();
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap();
    let space = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .space;
    let mapping = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(space) && mapping.range.is_some())
        .unwrap();
    (mapping.id, mapping.range.unwrap())
}
#[test]
fn scrolling() {
    let mut isolated = page(true);
    let mut reference = page(false);
    assert!(isolated.inspect().reuse.charged < reference.inspect().reuse.charged);
    assert_eq!(image(&mut isolated), image(&mut reference));
    for delta in [32, 16, 500, -1000, 64] {
        isolated.scroll(delta).unwrap();
        reference.scroll(delta).unwrap();
        assert_eq!(image(&mut isolated), image(&mut reference));
    }
}
#[test]
fn anchoring() {
    let content = |isolated| {
        let containment = if isolated {
            "contain:size layout paint;"
        } else {
            ""
        };
        let mut html = String::from("<body><div id='list' style='height:32px; overflow:auto;'>");
        for index in 0..20 {
            html.push_str(&format!(
                "<div style='height:32px; {containment}'>row {index}</div>"
            ));
        }
        html.push_str("</div></body>");
        html
    };
    let load = |html| {
        let mut page = Page::new(
            "app://test/index.html",
            Extent {
                width: 64,
                height: 32,
            },
        )
        .unwrap();
        page.load(&mut Source(html)).unwrap();
        page.evaluate(
            "const control = scroll.attach(document.getElementById('list')); control.followEnd();",
        )
        .unwrap();
        page.render().unwrap();
        page
    };
    let mut isolated = load(content(true));
    let mut reference = load(content(false));
    assert_eq!(image(&mut isolated), image(&mut reference));

    isolated.evaluate("const row = document.createElement('div'); row.setAttribute('style', 'height:32px; contain:size layout paint;'); row.setText('latest'); document.getElementById('list').appendChild(row);").unwrap();
    reference.evaluate("const row = document.createElement('div'); row.setAttribute('style', 'height:32px;'); row.setText('latest'); document.getElementById('list').appendChild(row);").unwrap();
    isolated.render().unwrap();
    reference.render().unwrap();
    let (isolated_space, isolated_range) = scroller(&mut isolated, "list");
    let (reference_space, reference_range) = scroller(&mut reference, "list");
    assert_eq!(isolated_range, 640);
    assert_eq!(reference_range, 640);
    assert_eq!(isolated.inspect().visual.offset(isolated_space), 640);
    assert_eq!(reference.inspect().visual.offset(reference_space), 640);
    assert_eq!(image(&mut isolated), image(&mut reference));
}
#[test]
fn queries() {
    let mut page = page(true);
    image(&mut page);
    let before = page.inspect();
    let geometry = page.geometry("child19").unwrap().unwrap();
    assert_eq!(geometry.y, 19 * 32);
    assert_eq!(geometry.height, 12);
    assert_eq!(before.scene, page.inspect().scene);
    assert!(page.frame().is_none());
    page.evaluate("document.getElementById('child19').setAttribute('style', 'height: 20px; background-color: #ff0000;');").unwrap();
    page.render().unwrap();
    assert_eq!(page.geometry("child19").unwrap().unwrap().height, 20);
    image(&mut page);
    page.scroll(19 * 32).unwrap();
    assert_eq!(image(&mut page).pixels[15 * 64 + 60], 0xff0000);
}
#[test]
fn lifecycle() {
    let mut page = page(true);
    let old = page.frame().unwrap();
    let expected = Cpu.render(&old).unwrap();
    page.acknowledge(old.id()).unwrap();
    let caches = page.inspect().caches;
    for cache in caches {
        page.cache(cache.id, shared::cache::Command::Clear).unwrap();
    }
    page.scroll(64).unwrap();
    image(&mut page);
    assert_eq!(Cpu.render(&old).unwrap(), expected);
    page.close();
    assert_eq!(Cpu.render(&old).unwrap(), expected);
}
#[test]
fn dimensions() {
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    let error = page
        .load(&mut Source(
            "<body><div style='contain: size layout paint'>text</div></body>".into(),
        ))
        .unwrap_err();
    assert!(error.to_string().contains("height or intrinsic estimate"));
}

#[test]
fn nested() {
    let html = "<body><div style='height:32px;'>top</div><div id='outer' style='height:64px; contain:size layout paint;'><div id='inner' style='height:32px; contain:size layout paint;'><input id='leaf' style='height:16px;'></div></div></body>";
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut Source(html.into())).unwrap();
    image(&mut page);
    let before = page.inspect().scene;
    assert_eq!(page.geometry("leaf").unwrap().unwrap().y, 32);
    assert_eq!(page.inspect().scene, before);
    page.scroll(32).unwrap();
    let frame = page.frame().unwrap();
    assert!(
        page.inspect()
            .fragments
            .iter()
            .any(|f| matches!(f.content, domengine::Content::Text { .. }))
    );
    page.acknowledge(frame.id()).unwrap();
    page.evaluate("document.getElementById('leaf').focus();")
        .unwrap();
    page.render().unwrap();
    page.scroll(-32).unwrap();
    let snapshot = page.inspect();
    assert!(
        snapshot
            .fragments
            .iter()
            .any(|f| f.node == snapshot.focused)
    );
}

#[test]
fn constraints() {
    let html = "<body><div style='display:flex; height:64px;'><div id='a' style='contain:size layout paint; flex-grow:1;'>first</div><div id='b' style='contain:size layout paint; flex-grow:1;'>second</div></div></body>";
    let mut page = Page::new(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
    )
    .unwrap();
    page.load(&mut Source(html.into())).unwrap();
    let rect = page.geometry("b").unwrap().unwrap();
    assert_eq!((rect.x, rect.width, rect.height), (32, 32, 64));
    image(&mut page);
}

#[test]
fn accounting() {
    let mut page = page(true);
    let snapshot = page.inspect();
    let initial = snapshot.computation;
    assert_eq!(snapshot.relevance.overscan, 16);
    assert_eq!(snapshot.relevance.boundaries, 20);
    assert!(snapshot.relevance.relevant < snapshot.relevance.boundaries);
    assert_eq!(snapshot.relevance.pending, snapshot.reuse.deferred);
    assert_eq!(
        (initial.layouts, initial.paints, initial.changes),
        (2, 2, 1)
    );
    page.render().unwrap();
    assert_eq!(page.inspect().computation, initial);
    page.geometry("child19").unwrap();
    let queried = page.inspect().computation;
    let query = queried.since(initial);
    assert_eq!((query.layouts, query.paints, query.queries), (1, 0, 1));
    assert_eq!((query.paint, query.scene), (0, 0));
    page.scroll(32).unwrap();
    let prefetched = page.inspect().computation;
    assert_eq!(prefetched.since(queried).layouts, 0);
    page.scroll(32).unwrap();
    let moved = page.inspect().computation;
    let movement = moved.since(queried);
    assert_eq!(
        (movement.layouts, movement.paints, movement.changes),
        (1, 1, 1)
    );
    assert_eq!(movement.checks, 3);
    page.render().unwrap();
    assert_eq!(page.inspect().computation, moved);
    let finality = page.inspect();
    let relevance = finality.relevance;
    let restored = webengine::wire::Bundle::new(finality).restore().unwrap();
    assert_eq!(restored.computation, moved);
    assert_eq!(restored.relevance, relevance);
}
