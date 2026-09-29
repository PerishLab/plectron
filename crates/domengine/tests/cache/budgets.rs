use super::{check, constraints, html};
use ::fixtures::Text;
use domengine::{Input, Layout, Pipeline, Sheet};

#[test]
fn pages() {
    let document = html::parse("<div><p>A</p><p>B</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let config = config::Store::default();
    config.set("layout.pages", 16.into());
    let mut pipeline =
        Pipeline::configured(Text, &shared::cache::Manager::default(), config.clone()).unwrap();
    let layout = pipeline.build(&input, constraints(128)).unwrap();
    assert_eq!(layout.snapshot().residency().width, 16);
    config.set("layout.pages", 0.into());
    assert!(pipeline.build(&input, constraints(128)).is_err());
}

#[test]
fn bounded() {
    let document = html::parse(&format!(
        "<div>{}</div>",
        (0..300)
            .map(|index| format!("<p>ABC{index}</p>"))
            .collect::<String>()
    ))
    .unwrap();
    let config = config::Store::default();
    config.set("layout.admission", usize::MAX.into());
    config.set("layout.recurrence", 0.into());
    let manager = shared::cache::Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let id = manager
        .inspect()
        .iter()
        .find(|cache| cache.name == "geometry")
        .unwrap()
        .id;
    manager
        .command(
            id,
            shared::cache::Command::Configure {
                policy: shared::cache::Policy::bounded(512, 4_194_304),
            },
        )
        .unwrap();
    check(&document, &mut pipeline, 128);
    assert!(pipeline.inspect().evicted > 0);
    assert!(pipeline.inspect().entries <= 512);
    assert!(pipeline.inspect().bytes <= 4_194_304);
    let work = pipeline.inspect().charged;
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(pipeline.inspect().charged, work);
    pipeline.enable(false);
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 0);
    assert_eq!(pipeline.inspect().disabled, pipeline.inspect().requests);
    assert_eq!(pipeline.inspect().charged, work);
    pipeline.enable(true);
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 1);
}

#[test]
fn work() {
    let mut source = "<p>A</p>".to_owned();
    let mut exhausted = false;
    for _ in 0..8 {
        source = format!("<div style='display: flex;'>{source}{source}</div>");
        let document = html::parse(&source).unwrap();
        let input = Input::capture(&document, &Sheet::default()).unwrap();
        let expected = Layout::build(&input, constraints(128), &Text);
        let mut pipeline = Pipeline::new(Text);
        for _ in 0..2 {
            let actual = pipeline.build(&input, constraints(128));
            exhausted |= compare(&expected, actual);
        }
        if exhausted {
            break;
        }
    }
    assert!(exhausted);
}

fn compare(
    expected: &Result<Layout, domengine::Fault>,
    actual: Result<Layout, domengine::Fault>,
) -> bool {
    match (expected, actual) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(expected.fragments(), actual.fragments());
            false
        }
        (Err(expected), Err(actual)) => {
            assert_eq!(*expected, actual);
            actual.to_string().contains("layout work budget")
        }
        _ => panic!("cache changed layout admission"),
    }
}

#[test]
fn admission() {
    use shared::cache::{Command, Manager};
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.recurrence", 0.into());
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let document = html::parse("<p>ABC</p>").unwrap();
    let caches = manager.inspect();
    let cache = caches
        .iter()
        .find(|cache| cache.name == "geometry")
        .unwrap();
    let mut policy = cache.target;
    policy.read = false;
    manager
        .command(cache.id, Command::Configure { policy })
        .unwrap();
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 0);
    assert!(pipeline.inspect().entries > 0);
    let work = pipeline.inspect().charged;
    policy.read = true;
    policy.write = false;
    manager
        .command(cache.id, Command::Configure { policy })
        .unwrap();
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 1);
    assert_eq!(pipeline.inspect().charged, work);
}

#[test]
fn recurrence() {
    let mut document = html::parse("<div id='value'></div>").unwrap();
    let value = document.find("value").unwrap();
    let config = config::Store::default();
    config.set("layout.recurrence", 1.into());
    let manager = shared::cache::Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    for index in 0..4 {
        document
            .attribute(value, "style", &format!("height: {index}px"))
            .unwrap();
        check(&document, &mut pipeline, 128);
    }
    assert!(pipeline.inspect().screened > 0);
    assert_eq!(pipeline.inspect().screenmax, 1);
}

#[test]
fn shaping() {
    let document = html::parse("<p>A</p>").unwrap();
    let config = config::Store::default();
    config.set("layout.recurrence", usize::MAX.into());
    let manager = shared::cache::Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    check(&document, &mut pipeline, 128);
    assert!(pipeline.inspect().screened > 0);
    assert!(pipeline.inspect().entries > 0);
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().shaping.shapes, 0);
    assert_eq!(pipeline.inspect().operations.texts, 0);
}

#[test]
fn window() {
    let document = html::parse(&format!(
        "<div style='display: flex;'>{}</div>",
        "<p>A</p>".repeat(8)
    ))
    .unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let build = |capacity: usize| {
        let config = config::Store::default();
        config.set("layout.retention", false.into());
        config.set("layout.window", capacity.into());
        let manager = shared::cache::Manager::default();
        let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
        let layout = pipeline.build(&input, constraints(128)).unwrap();
        (layout, pipeline.inspect())
    };
    let (baseline, plain) = build(0);
    let (candidate, windowed) = build(16);
    assert_eq!(candidate.extent(), baseline.extent());
    assert_eq!(candidate.fragments(), baseline.fragments());
    assert!(windowed.windowed > 0);
    assert!(windowed.windowpeak > 0);
    assert!(windowed.operations.blocks < plain.operations.blocks);
}

#[test]
fn transient() {
    let document = html::parse(&format!(
        "<div style='display: flex;'>{}</div>",
        "<div><p>A</p><p>B</p></div>".repeat(8)
    ))
    .unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let build = |capacity: usize| {
        let config = config::Store::default();
        config.set("layout.retention", false.into());
        config.set("layout.admission", 1.into());
        config.set("layout.recurrence", 0.into());
        config.set("layout.window", capacity.into());
        let manager = shared::cache::Manager::default();
        let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
        let layout = pipeline.build(&input, constraints(128)).unwrap();
        (layout, pipeline.inspect())
    };
    let (baseline, plain) = build(0);
    let (candidate, windowed) = build(16);
    assert_eq!(candidate.fragments(), baseline.fragments());
    assert!(windowed.bypassed > 0);
    assert!(windowed.windowed > 0);
    assert!(windowed.windowbytes > 0);
    assert!(windowed.operations.blocks < plain.operations.blocks);
    assert_eq!(windowed.entries, plain.entries);
    assert_eq!(windowed.bytes, plain.bytes);
}

#[test]
fn hierarchy() {
    let document = html::parse(&format!("<div>{}</div>", "<p>ABC</p>".repeat(50))).unwrap();
    let config = config::Store::default();
    let manager = shared::cache::Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    check(&document, &mut pipeline, 128);
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 2);
    assert_eq!(pipeline.inspect().bypassed, 2);
    assert_eq!(pipeline.inspect().shaping.preparations, 0);
    config.set("layout.admission", usize::MAX.into());
    check(&document, &mut pipeline, 128);
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(pipeline.inspect().hits, 1);
    config.set("layout.admission", 0.into());
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 0);
    assert!(pipeline.inspect().computed > 100);
    config.set("layout.admission", (-1).into());
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    assert!(
        pipeline
            .build(&input, constraints(128))
            .unwrap_err()
            .0
            .contains("layout.admission")
    );
    assert!(pipeline.inspect().failure.is_some());
    config.remove("layout.admission");
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 2);
}
