use domengine::Sheet;
use domengine::style::Resolver;
use domengine::style::animation::{Animations, Schedule};
use domengine::style::{Animation, Timing};
use domengine::{Capture, Constraints, Content, Extent, Layout, html};
use fixtures::Text;

#[test]
fn sampling() {
    let sheet = Sheet::parse(
        "@keyframes blink { from { opacity: 1; } 50% { opacity: 0; } to { opacity: 1; } }\n.pulse { animation-name: blink; animation-duration: 1200ms; animation-iteration-count: infinite; animation-timing-function: steps(1, end); }",
    )
    .unwrap();
    let frames = sheet.keyframes("blink").unwrap();
    let animation = Animation {
        name: Some("blink".into()),
        duration: 1_200_000,
        iterations: None,
        timing: Timing::Steps(1, false),
        ..Animation::default()
    };
    for (time, expected) in [
        (0, 255),
        (599_999, 255),
        (600_000, 0),
        (1_199_999, 0),
        (1_200_000, 255),
    ] {
        assert_eq!(frames.sample(&animation, time, 0, 255), expected);
    }
}

#[test]
fn scheduling() {
    let document = html::parse("<div id='pulse'></div>").unwrap();
    let node = document.find("pulse").unwrap();
    let frames =
        domengine::style::Keyframes::new("blink".into(), vec![(0.0, 255), (0.5, 0), (1.0, 255)])
            .unwrap();
    let stepped = Animation {
        name: Some("blink".into()),
        duration: 1_200_000,
        iterations: None,
        timing: Timing::Steps(1, false),
        ..Animation::default()
    };
    let mut animations = Animations::default();
    assert_eq!(animations.sample(node, &stepped, &frames, 255), 255);
    animations.discard();
    assert_eq!(animations.schedule(0), Some(Schedule::At(600_000)));
    assert_eq!(animations.schedule(700_000), Some(Schedule::At(700_000)));
    assert!(!animations.advance(599_999));
    assert!(animations.advance(600_000));
    assert_eq!(animations.changes()[0].opacity, 0);
    assert_eq!(animations.schedule(600_000), Some(Schedule::At(1_200_000)));
    let plans = animations.plans();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].target, node);
    assert_eq!(plans[0].plan.value(), 0);
    assert_eq!(plans[0].plan.sample(1_200_000), 255);

    let continuous = Animation {
        timing: Timing::Linear,
        ..stepped
    };
    assert_eq!(animations.sample(node, &continuous, &frames, 255), 0);
    assert_eq!(animations.schedule(600_000), Some(Schedule::Continuous));
}

#[test]
fn identity() {
    let mut sheet = Sheet::parse("@keyframes first {from {opacity: 1;} to {opacity: 0;}}").unwrap();
    let first = sheet.keyframes("first").unwrap().id;
    sheet
        .extend(Sheet::parse("@keyframes second {from {opacity: 0;} to {opacity: 1;}}").unwrap())
        .unwrap();
    let second = sheet.keyframes("second").unwrap().id;
    assert_ne!(first, second);
    sheet.set(first, 0.0, 0).unwrap();
    assert_eq!(sheet.keyframes("first").unwrap().revision, 1);
    assert_eq!(sheet.keyframes("second").unwrap().revision, 0);
    assert!(Sheet::parse(".pulse {animation-duration: 600000ms;}").is_ok());
    assert!(Sheet::parse(".pulse {animation-duration: 3600001ms;}").is_err());
}

#[test]
fn repaint() {
    let sheet = Sheet::parse(
        "@keyframes fade { from { opacity: 1; } to { opacity: 0; } }\n.pulse { width: 8px; height: 8px; background-color: #ff0000; animation-name: fade; animation-duration: 1s; animation-timing-function: linear; animation-iteration-count: infinite; }",
    ).unwrap();
    let document = html::parse("<div id='pulse' class='pulse'></div>").unwrap();
    let mut capture = Capture::new(Resolver::new(sheet));
    let extent = Extent {
        width: 16,
        height: 16,
    };
    let build = |capture: &mut Capture| {
        capture
            .update(&document, |input| {
                Layout::build(input, Constraints::viewport(extent), &Text)
            })
            .unwrap()
    };
    let opacity = |layout: &Layout| {
        layout
            .fragments()
            .iter()
            .find_map(|fragment| {
                (fragment.node == Some(document.find("pulse").unwrap().id())).then_some(
                    match fragment.content {
                        Content::Box { opacity, .. } => opacity,
                        _ => 255,
                    },
                )
            })
            .unwrap()
    };
    let initial = build(&mut capture);
    assert_eq!(opacity(&initial), 255);
    let tracks = initial.tracks(&capture.plans());
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].plan.value(), 255);
    assert_eq!(tracks[0].plan.sample(500_000), 128);
    assert!(capture.advance(500_000));
    assert_eq!(opacity(&build(&mut capture)), 128);
    assert_eq!(capture.inspect().reset, None);
    assert!(capture.advance(1_000_000));
    assert_eq!(opacity(&build(&mut capture)), 255);
    assert_eq!(capture.inspect().reset, None);
}

#[test]
fn caret() {
    let mut document = html::parse("<p-input id='editor'></p-input>").unwrap();
    let input = document.find("editor").unwrap();
    let text = document.text("").unwrap();
    let caret = document.element("p-caret").unwrap();
    document.append(input, text).unwrap();
    document.append(input, caret).unwrap();
    document.enable(input, text, caret).unwrap();
    let children = document.entry(input).unwrap().children.clone();
    assert_eq!(children.len(), 2);
    let caret = children[1];
    assert!(
        matches!(&document.entry(caret).unwrap().kind, domengine::Kind::Element(tag) if tag == "p-caret")
    );
    document.focus(Some(input)).unwrap();
    document.value(input, "中文").unwrap();
    let sheet = Sheet::parse("@keyframes blink {from {opacity: 1;} 50% {opacity: 0;} to {opacity: 1;}} p-caret {animation-name: blink; animation-duration: 1200ms; animation-iteration-count: infinite; animation-timing-function: steps(1, end);}").unwrap();
    let mut capture = Capture::new(Resolver::new(sheet));
    let layout = capture
        .update(&document, |source| {
            Layout::build(
                source,
                Constraints::viewport(Extent {
                    width: 120,
                    height: 60,
                }),
                &Text,
            )
        })
        .unwrap();
    assert!(layout.fragments().iter().any(|fragment| {
        fragment.node == Some(caret.id()) && matches!(fragment.content, Content::Box { .. })
    }));
    assert!(capture.advance(600_000));
    let layout = capture
        .update(&document, |source| {
            Layout::build(
                source,
                Constraints::viewport(Extent {
                    width: 120,
                    height: 60,
                }),
                &Text,
            )
        })
        .unwrap();
    assert!(layout.fragments().iter().any(|fragment| {
        fragment.node == Some(caret.id())
            && matches!(fragment.content, Content::Box { opacity: 0, .. })
    }));
    document.remove(input, caret).unwrap();
    let layout = capture
        .update(&document, |source| {
            Layout::build(
                source,
                Constraints::viewport(Extent {
                    width: 120,
                    height: 60,
                }),
                &Text,
            )
        })
        .unwrap();
    assert!(
        !layout
            .fragments()
            .iter()
            .any(|fragment| fragment.node == Some(caret.id()))
    );
    document.caret(input, 1).unwrap();
    assert_eq!(document.entry(caret).unwrap().parent, Some(input));
}
