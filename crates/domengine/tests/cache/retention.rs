use domengine::{Capture, Document, Layout, Pipeline, Sheet, html, style::Resolver};
use fixtures::Text;
use shared::cache::Manager;

fn build(
    capture: &mut Capture,
    pipeline: &mut Pipeline<Text>,
    document: &Document,
    width: usize,
) -> Layout {
    capture
        .update(document, |input| {
            let actual = pipeline.build(input, super::constraints(width))?;
            let expected = Layout::build(input, super::constraints(width), &Text)?;
            assert_eq!(actual.fragments(), expected.fragments());
            Ok(actual)
        })
        .unwrap()
}

#[test]
fn geometry() {
    let manager = Manager::default();
    let config = config::Store::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let mut capture = Capture::new(Resolver::managed(Sheet::default(), &manager).unwrap());
    let mut document = html::parse(&format!(
        "<div id='body'>{}</div>",
        "<p>Alpha</p>".repeat(80)
    ))
    .unwrap();
    let node = document.find("body").unwrap();
    let original = build(&mut capture, &mut pipeline, &document, 128);
    let pixels = super::pixels(&original);
    let work = pipeline.inspect().charged;
    for style in [
        "opacity: 0.5;",
        "opacity: 0; translate: 4px 2px;",
        "color: #ff0000;",
    ] {
        document.attribute(node, "style", style).unwrap();
        let layout = build(&mut capture, &mut pipeline, &document, 128);
        let report = pipeline.inspect();
        assert_eq!(report.retained, 1);
        assert_eq!(report.requests, 0);
        assert_eq!(report.charged, work);
        assert!(layout.snapshot().delta().changed().is_empty());
    }
    build(&mut capture, &mut pipeline, &document, 120);
    assert_eq!(pipeline.inspect().retained, 0);
    for style in ["padding: 8px;", "font-size: 24px;"] {
        document.attribute(node, "style", style).unwrap();
        build(&mut capture, &mut pipeline, &document, 120);
        assert_eq!(pipeline.inspect().retained, 0);
    }
    document.replace(node, "Different").unwrap();
    build(&mut capture, &mut pipeline, &document, 120);
    assert_eq!(pipeline.inspect().retained, 0);
    config.set("layout.retention", false.into());
    let rebuilt = build(&mut capture, &mut pipeline, &document, 120);
    assert_eq!(pipeline.inspect().retained, 0);
    assert!(rebuilt.snapshot().delta().changed().is_empty());
    config.set("layout.retention", true.into());
    pipeline.enable(false);
    build(&mut capture, &mut pipeline, &document, 120);
    assert_eq!(pipeline.inspect().retained, 0);
    pipeline.enable(true);
    pipeline.close();
    build(&mut capture, &mut pipeline, &document, 120);
    assert_eq!(pipeline.inspect().retained, 0);
    assert_eq!(super::pixels(&original), pixels);
}

#[test]
fn sequences() {
    let manager = Manager::default();
    let config = config::Store::default();
    config.set(
        "layout.sequences",
        serde_json::json!({"read": true, "write": true, "entries": 256, "bytes": 16_777_216}),
    );
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let mut capture = Capture::new(Resolver::managed(Sheet::default(), &manager).unwrap());
    let mut document = html::parse(&format!(
        "<div id='body' style='display: flex; flex-direction: column; height: 120px; gap: 3px;'>{}</div>",
        (0..65)
            .map(|index| format!(
                "<p id='p{index}' style='margin: 1px; flex-shrink: 0; overflow-wrap: anywhere;'>Text {index}</p>"
            ))
            .collect::<String>()
    ))
    .unwrap();
    let original = build(&mut capture, &mut pipeline, &document, 128);
    let pixels = super::pixels(&original);
    document
        .replace(document.find("p64").unwrap(), "New tail")
        .unwrap();
    build(&mut capture, &mut pipeline, &document, 128);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "sequences")
        .unwrap();
    assert!(cache.stats.hits >= 2);
    for (node, style, width) in [
        ("p0", "height: 23px; margin: 4px;", 128),
        ("p33", "display: none;", 128),
        ("p33", "height: 14px;", 96),
        ("p1", "color: #ff0000; opacity: 0.5;", 96),
    ] {
        document
            .attribute(
                document.find(node).unwrap(),
                "style",
                &format!("{style} flex-shrink: 0;"),
            )
            .unwrap();
        build(&mut capture, &mut pipeline, &document, width);
        let work = pipeline.inspect().charged;
        let mut reference = Pipeline::new(Text);
        reference.enable(false);
        super::check(&document, &mut reference, width);
        assert_eq!(work, reference.inspect().charged);
    }
    let body = document.find("body").unwrap();
    let first = document.find("p0").unwrap();
    document.append(body, first).unwrap();
    build(&mut capture, &mut pipeline, &document, 96);
    document.remove(body, first).unwrap();
    build(&mut capture, &mut pipeline, &document, 96);
    for style in [
        "flex-grow: 1;",
        "flex-shrink: 1;",
        "flex-shrink: 0; align-self: flex-end;",
        "contain: size layout paint; width: 24px; height: 20px; overflow: hidden;",
    ] {
        document
            .attribute(document.find("p1").unwrap(), "style", style)
            .unwrap();
        build(&mut capture, &mut pipeline, &document, 96);
    }
    config.set(
        "layout.sequences",
        serde_json::json!({"read": true, "write": true, "entries": 1, "bytes": 1}),
    );
    build(&mut capture, &mut pipeline, &document, 96);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "sequences")
        .unwrap();
    assert_eq!(cache.stats.entries, 0);
    assert!(cache.stats.evicted > 0);
    pipeline.enable(false);
    build(&mut capture, &mut pipeline, &document, 96);
    pipeline.close();
    assert_eq!(super::pixels(&original), pixels);
}
