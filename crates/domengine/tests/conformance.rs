use domengine::{Document, Extent, Layout, Sheet, html, raster};

#[test]
fn identity() {
    let mut document = Document::default();
    let parent = document.element("div").unwrap();
    let child = document.element("button").unwrap();
    document.append(document.root(), parent).unwrap();
    document.append(parent, child).unwrap();
    document.attribute(child, "id", "button").unwrap();
    assert!(document.append(child, parent).is_err());
    document.remove(parent, child).unwrap();
    assert!(!document.connected(child).unwrap());
    assert!(document.find("button").is_none());
    document.append(document.root(), child).unwrap();
    assert_eq!(document.find("button"), Some(child));
    let detached = document.element("div").unwrap();
    assert!(document.append(detached, document.root()).is_err());
    let foreign = Document::default();
    assert!(foreign.entry(child).is_err());
    assert!(document.append(parent, foreign.root()).is_err());
}

#[test]
fn pipeline() {
    let mut document =
        html::parse("<body><div id='first'>A</div><button id='second'>B</button></body>").unwrap();
    let sheet = Sheet::parse("body {padding: 8px;} div {height: 20px; background-color: #ff0000;} #second {height: 24px; background-color: #0000ff;}").unwrap();
    let extent = Extent {
        width: 100,
        height: 100,
    };
    let layout = build(&document, &sheet, extent).unwrap();
    let first = document.find("first").unwrap();
    let second = document.find("second").unwrap();
    assert_eq!(layout.hit(80, 10), Some(first));
    assert_eq!(layout.hit(80, 30), Some(second));
    let frame = raster(extent, &layout.paint()).unwrap();
    assert_eq!(frame.pixels[10 * 100 + 80], 0xff0000);
    assert_eq!(frame.pixels[30 * 100 + 80], 0x0000ff);
    document.attribute(first, "style", "height: 40px;").unwrap();
    let changed = build(&document, &sheet, extent).unwrap();
    assert_eq!(changed.hit(80, 30), Some(first));
    assert_eq!(changed.hit(80, 50), Some(second));
    assert!(changed.revision() > layout.revision());
    document
        .attribute(first, "style", "display: none;")
        .unwrap();
    let hidden = build(&document, &sheet, extent).unwrap();
    assert_eq!(document.find("first"), Some(first));
    assert!(
        !hidden
            .fragments()
            .iter()
            .any(|item| item.node == Some(first.id()))
    );
}

#[test]
fn subset() {
    for source in [
        "<div>",
        "<div></p>",
        "<div onclick='x'></div>",
        "<iframe></iframe>",
        "<p>&unknown;</p>",
    ] {
        assert!(html::parse(source).is_err(), "{source}");
    }
    for css in [
        "div {position: absolute;}",
        "div p {color: #ffffff;}",
        "div {width: 2%;}",
    ] {
        assert!(Sheet::parse(css).is_err(), "{css}");
    }
    let document = html::parse("<!doctype html><!-- comment --><p>A &amp; B</p>").unwrap();
    let layout = build(
        &document,
        &Sheet::default(),
        Extent {
            width: 48,
            height: 48,
        },
    )
    .unwrap();
    let frame = raster(
        Extent {
            width: 48,
            height: 48,
        },
        &layout.paint(),
    )
    .unwrap();
    assert!(frame.pixels.iter().any(|pixel| *pixel != 0xffffff));
}

#[test]
fn fragments() {
    let document = html::parse("<!DoCtYpE hTmL><!--雪--><DIV\u{2003}ID = '雪&amp;🦀'> A\u{2003}B &lt; C <span> D </span> E </DIV >").unwrap();
    let node = document.find("雪&🦀").unwrap();
    let entry = document.entry(node).unwrap();
    assert_eq!(entry.children.len(), 3);
    assert!(
        matches!(&document.entry(entry.children[0]).unwrap().kind, domengine::Kind::Text(text) if text == " A\u{2003}B < C ")
    );
    assert!(
        matches!(&document.entry(entry.children[2]).unwrap().kind, domengine::Kind::Text(text) if text == " E ")
    );
    let span = document.entry(entry.children[1]).unwrap();
    assert!(
        matches!(&document.entry(span.children[0]).unwrap().kind, domengine::Kind::Text(text) if text == " D ")
    );
}

#[test]
fn malformed() {
    for source in [
        "<!--雪",
        "<!DOCTYPE  html>",
        "<div/>",
        "<div id=bare></div>",
        "<div id='雪></div>",
        "<div id='a' ID='b'></div>",
        "<div>&amp</div>",
        "<div>&#65;</div>",
        "<div></div",
        "<div></>",
    ] {
        assert!(html::parse(source).is_err(), "{source}");
    }
    let error = html::parse("雪<div id=bare></div>").err().unwrap();
    assert!(error.0.contains("byte 11"), "{error}");
    for source in [
        "div {width 1px;}",
        "div {width: 1px",
        "div {color: #ffffff} trailing",
        "div, p {color: #ffffff;}",
        "div:hover {}",
        "/* comment */ div {}",
        "div {color: red;}",
        "div {width: 1em;}",
        "div {display: grid;}",
    ] {
        assert!(Sheet::parse(source).is_err(), "{source}");
    }
}

#[test]
fn cascade() {
    let mut document =
        html::parse("<div id='item' class='a b'><p id='child'>text</p></div>").unwrap();
    let node = document.find("item").unwrap();
    let sheet = Sheet::parse("\u{2003}#item {color: #010203; width: 2px;} .a {color: #aaaaaa;} div {color: #bbbbbb; width: 1px;} #item {width: 3px;} .b {height: 7px; ;}").unwrap();
    let style = sheet
        .compute(
            &document,
            node,
            domengine::style::Inherited {
                color: 0xffffff,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        (style.color, style.width, style.height),
        (0x010203, Some(3), Some(7))
    );
    let child = sheet
        .compute(
            &document,
            document.find("child").unwrap(),
            domengine::style::Inherited::from(&style),
        )
        .unwrap();
    assert_eq!(child.color, 0x010203);
    document
        .attribute(node, "style", "color: #fefefe; width: 4px; width: 5px")
        .unwrap();
    let inline = sheet
        .compute(
            &document,
            node,
            domengine::style::Inherited {
                color: 0,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!((inline.color, inline.width), (0xfefefe, Some(5)));
}

#[test]
fn budgets() {
    assert!(html::parse(&" ".repeat(65_537)).is_err());
    assert!(Sheet::parse(&" ".repeat(65_537)).is_err());
    assert!(Sheet::parse(&"div {}".repeat(257)).is_err());
    let source = format!("<!--{}--><p>雪</p>", " ".repeat(60_000));
    assert!(html::parse(&source).is_ok());
}

#[test]
fn excluded() {
    for source in ["div {float: left;}", "div {clear: both;}"] {
        let error = Sheet::parse(source).err().unwrap();
        assert!(error.0.contains("unsupported CSS property"), "{error}");
    }
}

fn build(
    document: &domengine::Document,
    sheet: &domengine::Sheet,
    extent: domengine::Extent,
) -> Result<Layout, domengine::Fault> {
    Layout::build(
        &domengine::Input::capture(document, sheet)?,
        domengine::Constraints::viewport(extent),
        &domengine::Text,
    )
}

#[test]
fn fonts() {
    let sheet = Sheet::parse("@font-face {font-family: 'Body Face'; font-weight: 700; src: url('../fonts/body.ttf');} p {font-family: 'Body Face';}").unwrap();
    assert_eq!(sheet.fonts().len(), 1);
    assert_eq!(sheet.fonts()[0].family, "Body Face");
    assert_eq!(sheet.fonts()[0].weight, textengine::Weight::Bold);
    assert_eq!(sheet.fonts()[0].source, "../fonts/body.ttf");
    for source in [
        "@font-face {font-family: body;}",
        "@font-face {src: url(body.ttf);}",
        "@font-face {font-family: body; src: local(body);}",
        "@font-face {font-family: body; src: url(a), url(b);}",
        "@font-face {font-family: body; src: url(a); font-display: swap;}",
        "p {src: url(body.ttf);}",
    ] {
        assert!(Sheet::parse(source).is_err(), "{source}");
    }
}
