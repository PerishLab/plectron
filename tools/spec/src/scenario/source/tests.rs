use super::*;
use webengine::Page;

#[test]
fn artifacts() {
    let mut source = Source::default();
    for path in [
        "layers/main.ts",
        "convergence/main.ts",
        "scroll/main.ts",
        "spaces/main.ts",
        "visual/main.ts",
        "typescript/main.ts",
    ] {
        let url = webengine::resource::address(&format!("app://scenarios/{path}")).unwrap();
        let script = source.script(&url).unwrap();
        assert_eq!(script.response.mime, "text/javascript");
        assert!(script.source.is_some());
        let mut workspace = project::Workspace::open(
            crate::root().join("apps").join(path),
            project::Adapters::new(typescript::Typescript, pt::Templates),
        )
        .unwrap();
        let output = workspace.link(project::Target::Script).unwrap();
        assert_eq!(script.response.body, output.javascript.as_bytes());
        let assets = source.assets().unwrap();
        assert_eq!(assets[url.as_str()].bytes, script.response.body.len());
        assert_eq!(
            assets[url.as_str()].fingerprint,
            super::fingerprint(&script.response.body)
        );
        assert_eq!(assets[url.as_str()].requests, 1);
        let removed = webengine::resource::address(&format!(
            "app://scenarios/{}",
            path.replace(".ts", ".js")
        ))
        .unwrap();
        assert!(source.script(&removed).is_err());
    }
    assert_eq!(source.assets().unwrap().len(), 6);
}

fn page(source: &mut Source) -> Page {
    let mut page = Page::new(
        "app://scenarios/typescript/index.html",
        domengine::Extent {
            width: 640,
            height: 400,
        },
    )
    .unwrap();
    page.load(source).unwrap();
    page
}

#[test]
fn typescript() {
    let mut source = Source::default();
    let mut page = page(&mut source);
    page.collect().unwrap();
    page.receive("increment".into()).unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["count:1"]);
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Count: 1")
    );
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
            .starts_with("apps/typescript/model.ts:")
    );
    page.receive("asynchronous".into()).unwrap();
    page.render().unwrap();
    assert!(page.inspect().jobs.rejected[0].starts_with("apps/typescript/model.ts:"));
    page.close();
    let assets = source.assets().unwrap();
    assert_eq!(assets.len(), 3);
    assert!(assets.values().all(|asset| asset.requests == 1));
    assert!(assets.keys().any(|url| url.ends_with("main.ts")));
}

#[test]
fn fingerprint() {
    let source = Source::default();
    source
        .record("app://scenarios/test.txt".into(), b"first", 64)
        .unwrap();
    assert!(
        source
            .record("app://scenarios/test.txt".into(), b"other", 64)
            .is_err()
    );
    let url = webengine::resource::address("app://scenarios/typescript/style.css").unwrap();
    let request = Request::new(url.clone(), webengine::resource::Kind::Text);
    let response = source.send(&request, &Cancellation::default()).unwrap();
    assert_eq!(
        source.assets.lock().unwrap()[url.as_str()].fingerprint,
        super::fingerprint(&response.body)
    );
    let mut source = Source::default();
    source.request(&request).unwrap();
    source
        .assets
        .lock()
        .unwrap()
        .get_mut(url.as_str())
        .unwrap()
        .fingerprint ^= 1;
    assert!(
        source
            .assets()
            .unwrap_err()
            .0
            .contains("changed during run")
    );
}

#[test]
fn confinement() {
    let mut source = Source::default();
    for url in [
        "app://elsewhere/typescript/main.ts",
        "http://scenarios/typescript/main.ts",
        "app://scenarios/%2e%2e/Cargo.toml",
    ] {
        let url = webengine::resource::address(url).unwrap();
        assert!(source.script(&url).is_err());
    }
    let url = webengine::resource::address("app://scenarios/typescript/main.ts").unwrap();
    let request = Request::new(url, webengine::resource::Kind::Text);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(source.send(&request, &cancellation).is_err());
    assert!(source.assets.lock().unwrap().is_empty());
}
