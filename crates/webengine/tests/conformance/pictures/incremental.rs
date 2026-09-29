use super::{Extent, Page, png, source};
fn page() -> Page {
    let config = config::Store::default();
    config.set("dom.history", 1024.into());
    let mut page = Page::configured(
        "app://images/index.html",
        Extent {
            width: 64,
            height: 64,
        },
        config,
    )
    .unwrap();
    page.load(&mut source("image/png", png(8, 4, [255, 0, 0, 255])))
        .unwrap();
    page.evaluate(&fixtures::script("document", "const root = document.getElementById('root'); const first = document.getElementById('first');")).unwrap();
    page
}
#[test]
fn discovery() {
    let mut page = page();
    let baseline = page.inspect().images;
    for _ in 0..8 {
        page.render().unwrap();
    }
    assert_eq!(page.inspect().images.visited, baseline.visited);
    page.evaluate("root.setAttribute('style', 'color: #aa0000;');")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().images.visited, baseline.visited);
    page.evaluate("first.setAttribute('src', 'two.png'); first.setAttribute('src', 'one.png'); first.setAttribute('src', 'two.png');").unwrap();
    page.render().unwrap();
    let changed = page.inspect().images;
    assert_eq!(changed.visited, baseline.visited + 1);
    assert_eq!(changed.candidates, baseline.candidates + 1);
    assert_eq!(changed.fetches, baseline.fetches + 1);
    page.evaluate("const box = document.createElement('div'); const child = document.createElement('img'); child.setAttribute('src', 'missing.png'); box.appendChild(child);").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().images.fetches, changed.fetches);
    page.evaluate("child.setAttribute('src', 'one.png'); root.appendChild(box);")
        .unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().images.bindings, changed.bindings + 1);
    assert_eq!(page.inspect().images.scans, baseline.scans);
}
#[test]
fn recovery() {
    let mut page = page();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    page.evaluate("first.setAttribute('src', 'missing.png');")
        .unwrap();
    assert!(page.render().is_err());
    let failed = page.inspect().images;
    assert!(page.render().is_err());
    assert_eq!(page.inspect().images.visited, failed.visited);
    assert_eq!(page.inspect().images.fetches, failed.fetches + 1);
    page.evaluate("root.removeChild(first);").unwrap();
    page.render().unwrap();
    page.evaluate("first.setAttribute('src', 'two.png'); root.appendChild(first);")
        .unwrap();
    page.render().unwrap();
    let before = page.inspect().images;
    for index in 0..1025 {
        page.evaluate(&format!("root.setAttribute('class', 'a{index}');"))
            .unwrap();
    }
    page.evaluate("first.setAttribute('src', 'one.png');")
        .unwrap();
    page.render().unwrap();
    let recovered = page.inspect().images;
    assert_eq!(recovered.scans, before.scans + 1);
    assert_eq!(recovered.bindings, before.bindings + 1);
    assert!(recovered.visited > before.visited);
}
