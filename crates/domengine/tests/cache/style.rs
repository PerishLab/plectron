use super::{constraints, pixels};
use ::fixtures::Text;
use domengine::{
    Document, Input, Layout, Pipeline, Sheet, html,
    style::{Inherited, Resolver, Subject},
};
use shared::cache::{Command, Manager, Policy, Snapshot};

fn sheet() -> Sheet {
    Sheet::parse("p { width: 24px; } .red { color: #ff0000; } #child { height: 16px; }").unwrap()
}
fn capture(document: &Document, resolver: &mut Resolver, pipeline: &mut Pipeline<Text>) -> Layout {
    let input = Input::resolve(document, resolver).unwrap();
    let actual = pipeline.build(&input, constraints(128)).unwrap();
    let input = Input::capture(document, &sheet()).unwrap();
    let expected = Layout::build(&input, constraints(128), &Text).unwrap();
    assert_eq!(actual.fragments(), expected.fragments());
    assert_eq!(pixels(&actual), pixels(&expected));
    actual
}
fn named(manager: &Manager, name: &str) -> Snapshot {
    manager
        .inspect()
        .into_iter()
        .find(|entry| entry.name == name)
        .unwrap()
}

#[test]
fn dependencies() {
    let manager = Manager::default();
    let mut resolver = Resolver::managed(sheet(), &manager).unwrap();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let mut document =
        html::parse("<div id='root'><p id='child'>ABC</p><p class='red'>DEF</p></div>").unwrap();
    let first = capture(&document, &mut resolver, &mut pipeline);
    let work = pipeline.inspect().charged;
    let matching = named(&manager, "matching");
    let computed = named(&manager, "computed");
    let child = document.find("child").unwrap();
    document.replace(child, "XYZ").unwrap();
    capture(&document, &mut resolver, &mut pipeline);
    assert_eq!(
        named(&manager, "matching").stats.inserted,
        matching.stats.inserted
    );
    assert_eq!(
        named(&manager, "computed").stats.inserted,
        computed.stats.inserted
    );
    let root = document.find("root").unwrap();
    document
        .attribute(root, "style", "color: #00ff00;")
        .unwrap();
    let green = capture(&document, &mut resolver, &mut pipeline);
    assert_eq!(
        named(&manager, "matching").stats.inserted,
        matching.stats.inserted + 1
    );
    assert!(named(&manager, "computed").stats.inserted > computed.stats.inserted);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(pipeline.inspect().charged, work);
    assert_ne!(pixels(&first), pixels(&green));
    document.attribute(child, "class", "red").unwrap();
    capture(&document, &mut resolver, &mut pipeline);
    document.attribute(child, "id", "other").unwrap();
    capture(&document, &mut resolver, &mut pipeline);
    document
        .attribute(child, "style", "width: 32px; color: #0000ff;")
        .unwrap();
    capture(&document, &mut resolver, &mut pipeline);
    assert!(pipeline.inspect().computed > 0);
}

#[test]
fn lifecycle() {
    let manager = Manager::default();
    let mut resolver = Resolver::managed(sheet(), &manager).unwrap();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let document = html::parse("<div><p id='child'>ABC</p><p class='red'>DEF</p></div>").unwrap();
    let original = capture(&document, &mut resolver, &mut pipeline);
    let work = pipeline.inspect().charged;
    let disabled = Policy {
        read: false,
        write: false,
        entries: 512,
        bytes: 4_194_304,
    };
    for command in [
        Command::Configure { policy: disabled },
        Command::Configure {
            policy: Policy::bounded(1, 512),
        },
        Command::Clear,
        Command::Configure {
            policy: Policy::bounded(512, 4_194_304),
        },
        Command::Destroy,
    ] {
        for cache in manager.inspect() {
            manager.command(cache.id, command).unwrap();
        }
        let actual = capture(&document, &mut resolver, &mut pipeline);
        assert_eq!(actual.fragments(), original.fragments());
        assert_eq!(pipeline.inspect().charged, work);
        assert!(
            manager
                .inspect()
                .iter()
                .all(|cache| cache.requested == cache.applied)
        );
    }
    assert!(
        manager
            .inspect()
            .iter()
            .all(|cache| cache.closed && cache.stats.entries == 0)
    );
    assert_eq!(pipeline.inspect().hits, 0);
    assert_eq!(pipeline.inspect().disabled, pipeline.inspect().requests);
    drop(resolver);
    drop(pipeline);
    assert!(manager.inspect().is_empty());
    assert!(!pixels(&original).pixels.is_empty());
}

#[test]
fn cascade() {
    let mut resolver = Resolver::new(Sheet::parse("p { color: #0000ff; } .red { color: #ff0000; } #child { color: #00ff00; } .red { width: 32px; }").unwrap());
    let mut document =
        html::parse("<p id='child' class='red' style='color: #112233;'>ABC</p>").unwrap();
    let node = document.find("child").unwrap();
    let inherited = Inherited {
        color: 0xffffff,
        ..Inherited::default()
    };
    let style = resolver
        .resolve(
            Subject::capture(&document, node).unwrap(),
            inherited.clone(),
        )
        .unwrap();
    assert_eq!(style.color, 0x112233);
    assert_eq!(style.width, Some(32));
    document.attribute(node, "style", "").unwrap();
    let subject = Subject::capture(&document, node).unwrap();
    assert_eq!(
        resolver
            .resolve(subject.clone(), inherited.clone())
            .unwrap()
            .color,
        0x00ff00
    );
    resolver.replace(Sheet::parse("p { color: #445566; }").unwrap());
    assert_eq!(
        resolver.resolve(subject, inherited.clone()).unwrap().color,
        0x445566
    );
    assert_eq!(style.color, 0x112233);
    let invalid = Subject {
        direction: None,
        tag: Some("p".into()),
        id: None,
        classes: None,
        inline: Some("width: broken;".into()),
    };
    for _ in 0..2 {
        assert!(
            resolver
                .resolve(invalid.clone(), inherited.clone())
                .is_err()
        );
    }
    assert_eq!(
        resolver
            .resolve(
                Subject::capture(&document, node).unwrap(),
                inherited.clone()
            )
            .unwrap()
            .color,
        0x445566
    );
}

#[test]
fn direction() {
    let manager = Manager::default();
    let mut resolver = Resolver::managed(sheet(), &manager).unwrap();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let mut document =
        html::parse("<div id='root'><p style='width: 96px;'>AB אב</p></div>").unwrap();
    let root = document.find("root").unwrap();
    let mut previous = capture(&document, &mut resolver, &mut pipeline);
    for direction in ["rtl", "ltr", "rtl", "auto"] {
        document.attribute(root, "dir", direction).unwrap();
        let actual = capture(&document, &mut resolver, &mut pipeline);
        assert!(
            pixels(&actual) != pixels(&previous),
            "direction: {direction}"
        );
        previous = actual;
    }
    document.replace(root, "\u{202e}AB").unwrap();
    let input = Input::resolve(&document, &mut resolver).unwrap();
    assert!(pipeline.build(&input, constraints(128)).is_err());
    assert!(!pixels(&previous).pixels.is_empty());
    document.replace(root, "אב").unwrap();
    capture(&document, &mut resolver, &mut pipeline);
}

#[test]
fn wrapping() {
    let manager = Manager::default();
    let mut resolver = Resolver::managed(sheet(), &manager).unwrap();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let mut document =
        html::parse("<div id='root'><p style='width: 48px;'>中文文字 새로운 글</p></div>").unwrap();
    let root = document.find("root").unwrap();
    let before = capture(&document, &mut resolver, &mut pipeline);
    document
        .attribute(root, "style", "word-break: keep-all;")
        .unwrap();
    let after = capture(&document, &mut resolver, &mut pipeline);
    assert!(pixels(&before) != pixels(&after));
    document
        .attribute(root, "style", "word-break: normal;")
        .unwrap();
    let restored = capture(&document, &mut resolver, &mut pipeline);
    assert_eq!(pixels(&before), pixels(&restored));
    document.replace(root, "A\u{200b}B\u{2028}中文").unwrap();
    capture(&document, &mut resolver, &mut pipeline);
}

#[test]
fn continuation() {
    let manager = Manager::default();
    let mut resolver = Resolver::managed(sheet(), &manager).unwrap();
    let mut pipeline = super::managed(&manager);
    let mut document = html::parse("<p id='text' dir='rtl' style='font-family: lato, noto-arabic; overflow-wrap: anywhere;'>بِبَ\u{200b}بُبِ</p>").unwrap();
    let node = document.find("text").unwrap();
    let before = capture(&document, &mut resolver, &mut pipeline);
    document
        .attribute(
            node,
            "style",
            "font-family: lato, noto-arabic; overflow-wrap: anywhere; width: 12px;",
        )
        .unwrap();
    let after = capture(&document, &mut resolver, &mut pipeline);
    assert_ne!(pixels(&before), pixels(&after));
    capture(&document, &mut resolver, &mut pipeline);
    assert!(pipeline.inspect().hits > 0);
    document
        .attribute(
            node,
            "style",
            "font-family: lato, noto-arabic; overflow-wrap: anywhere;",
        )
        .unwrap();
    let restored = capture(&document, &mut resolver, &mut pipeline);
    assert_eq!(pixels(&before), pixels(&restored));
    document.replace(node, "ffi بلا").unwrap();
    let mut images = Vec::new();
    for width in [16, 80, 16] {
        document
            .attribute(
                node,
                "style",
                &format!(
                    "font-family: lato, noto-arabic; overflow-wrap: anywhere; width: {width}px;"
                ),
            )
            .unwrap();
        images.push(pixels(&capture(&document, &mut resolver, &mut pipeline)));
    }
    assert_eq!(images[0], images[2]);
    assert_ne!(images[0], images[1]);
}
