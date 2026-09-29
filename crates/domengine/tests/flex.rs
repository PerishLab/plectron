use ::fixtures::Text;
use domengine::{Constraints, Content, Document, Extent, Input, Layout, Rect, Sheet, html};

fn layout(html: &str, css: &str) -> (Document, Layout) {
    let document = html::parse(html).unwrap();
    let input = Input::capture(&document, &Sheet::parse(css).unwrap()).unwrap();
    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 320,
            height: 200,
        }),
        &Text,
    )
    .unwrap();
    (document, layout)
}
fn rect((document, layout): &(Document, Layout), name: &str) -> Rect {
    let node = document.find(name).unwrap();
    layout
        .fragments()
        .iter()
        .find(|fragment| fragment.node == Some(node.id()))
        .unwrap()
        .rect
}
const PAIR: &str = "<div id='parent'><div id='a'></div><div id='b'></div></div>";

#[test]
fn growth() {
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 210px; height: 40px; gap: 10px;} #a {flex-basis: 20px; flex-grow: 1;} #b {flex-basis: 20px; flex-grow: 1;}",
    );
    assert_eq!(
        rect(&page, "a"),
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 40
        }
    );
    assert_eq!(
        rect(&page, "b"),
        Rect {
            x: 110,
            y: 0,
            width: 100,
            height: 40
        }
    );
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 200px;} #a {flex-basis: 0; flex-grow: 0.25;} #b {flex-basis: 0; flex-grow: 0.25;}",
    );
    assert_eq!(rect(&page, "a").width, 50);
    assert_eq!(rect(&page, "b").width, 50);
}
#[test]
fn shrinking() {
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 150px;} #a {width: 100px;} #b {width: 200px;}",
    );
    assert_eq!(rect(&page, "a").width, 50);
    assert_eq!(rect(&page, "b").width, 100);
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 150px;} #a {width: 100px; flex-shrink: 0;} #b {width: 200px;}",
    );
    assert_eq!(rect(&page, "a").width, 100);
    assert_eq!(rect(&page, "b").width, 50);
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 240px;} #a {width: 100px; flex-shrink: 0.25;} #b {width: 200px; flex-shrink: 0.25;}",
    );
    assert_eq!(rect(&page, "a").width, 90);
    assert_eq!(rect(&page, "b").width, 180);
}
#[test]
fn freezing() {
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 180px;} #a {flex-basis: 100px; min-width: 120px;} #b {flex-basis: 100px;}",
    );
    assert_eq!(rect(&page, "a").width, 120);
    assert_eq!(rect(&page, "b").width, 60);
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 200px;} #a {flex-basis: 20px; flex-grow: 1; max-width: 60px;} #b {flex-basis: 20px; flex-grow: 1;}",
    );
    assert_eq!(rect(&page, "a").width, 60);
    assert_eq!(rect(&page, "b").width, 140);
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 200px;} #a {flex-basis: 0; min-width: 80px; max-width: 40px;} #b {flex-grow: 1;}",
    );
    assert_eq!(rect(&page, "a").width, 80);
    assert_eq!(rect(&page, "b").width, 120);
}
#[test]
fn alignment() {
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 101px; height: 60px; gap: 5px; justify-content: space-between; align-items: center;} #a {width: 20px; height: 20px;} #b {width: 30px; height: 30px; align-self: flex-end;}",
    );
    assert_eq!(
        rect(&page, "a"),
        Rect {
            x: 0,
            y: 20,
            width: 20,
            height: 20
        }
    );
    assert_eq!(
        rect(&page, "b"),
        Rect {
            x: 71,
            y: 30,
            width: 30,
            height: 30
        }
    );
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 101px; gap: 4px; justify-content: center;} #a {width: 20px;} #b {width: 20px;}",
    );
    assert_eq!(rect(&page, "a").x, 28);
    assert_eq!(rect(&page, "b").x, 52);
}
#[test]
fn boxes() {
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 120px; height: 40px; padding: 5px; margin: 3px; gap: 4px;} #a {flex-basis: 0; flex-grow: 1; padding: 2px; margin: 1px;} #b {flex-basis: 0; flex-grow: 1; padding: 2px; margin: 1px;}",
    );
    assert_eq!(
        rect(&page, "parent"),
        Rect {
            x: 3,
            y: 3,
            width: 130,
            height: 50
        }
    );
    assert_eq!(
        rect(&page, "a"),
        Rect {
            x: 9,
            y: 9,
            width: 56,
            height: 38
        }
    );
    assert_eq!(
        rect(&page, "b"),
        Rect {
            x: 71,
            y: 9,
            width: 56,
            height: 38
        }
    );
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 101px;} #a {flex-grow: 1;} #b {flex-grow: 1;}",
    );
    assert_eq!(rect(&page, "a").width + rect(&page, "b").width, 101);
    assert_eq!(rect(&page, "b").x, rect(&page, "a").width);
}
#[test]
fn text() {
    let page = layout(
        "<div id='parent'><p id='a'>ABCD</p><p id='b'>EFGH</p></div>",
        "#parent {display: flex; overflow-wrap: anywhere; width: 48px;}",
    );
    assert_eq!(rect(&page, "a").width, 24);
    assert_eq!(rect(&page, "a").height, 64);
    assert_eq!(rect(&page, "b").x, 24);
    assert_eq!(
        page.1
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment.content, Content::Line))
            .count(),
        8
    );
    assert_eq!(page.1.hit(30, 20), page.0.find("b"));
    let page = layout(
        "<div id='parent'><p id='a'>AB</p><p id='b'>CD</p></div>",
        "#parent {display: flex; overflow-wrap: anywhere; width: 8px;}",
    );
    assert_eq!(rect(&page, "a").width, 16);
    assert_eq!(rect(&page, "b").width, 16);
    let page = layout(
        "<div id='parent'><p id='a'>AB</p><p id='b'>CD</p></div>",
        "#parent {display: flex; overflow-wrap: anywhere; width: 8px;} p {min-width: 0;}",
    );
    assert_eq!(rect(&page, "a").width, 4);
    assert_eq!(rect(&page, "b").width, 4);
}
#[test]
fn columns() {
    let page = layout(
        PAIR,
        "#parent {display: flex; flex-direction: column; width: 100px; height: 100px; gap: 10px;} #a {flex-grow: 1;} #b {flex-grow: 2;}",
    );
    assert_eq!(
        rect(&page, "a"),
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30
        }
    );
    assert_eq!(
        rect(&page, "b"),
        Rect {
            x: 0,
            y: 40,
            width: 100,
            height: 60
        }
    );
    let page = layout(
        PAIR,
        "#parent {display: flex; flex-direction: column; gap: 10px;} #a {height: 20px; flex-grow: 1;} #b {height: 30px; flex-grow: 2;}",
    );
    assert_eq!(rect(&page, "parent").height, 60);
    assert_eq!(rect(&page, "a").height, 20);
    assert_eq!(rect(&page, "b").y, 30);
}
#[test]
fn nesting() {
    let page = layout(
        "<div id='parent'><div id='a'><p id='label'>AB</p><div id='fill'></div></div><div id='b'></div></div>",
        "#parent {display: flex; width: 100px; height: 100px;} #a {display: flex; flex-direction: column; flex-basis: 0; flex-grow: 1; min-width: 0;} #fill {flex-grow: 1;} #b {flex-basis: 0; flex-grow: 1;}",
    );
    assert_eq!(rect(&page, "a").width, 50);
    assert_eq!(rect(&page, "label").height, 16);
    assert_eq!(
        rect(&page, "fill"),
        Rect {
            x: 0,
            y: 16,
            width: 50,
            height: 84
        }
    );
    let page = layout(
        "<div id='parent'><div id='a'></div><div style='display: none;'></div><div id='b'></div></div>",
        "#parent {display: flex; gap: 10px;} #a {width: 20px;} #b {width: 30px;}",
    );
    assert_eq!(rect(&page, "b").x, 30);
}
#[test]
fn rejection() {
    for property in [
        "flex-wrap: wrap",
        "flex-direction: row-reverse",
        "align-items: baseline",
        "align-items: space-between",
        "justify-content: stretch",
        "flex-basis: 10%",
        "gap: -1px",
        "flex-grow: NaN",
        "flex-shrink: -1",
        "flex: 1",
        "order: 2",
    ] {
        assert!(
            Sheet::parse(&format!("div {{{property};}}")).is_err(),
            "{property}"
        );
    }
    let page = layout(
        PAIR,
        "#parent {display: flex; width: 20px; justify-content: safe center;} #a {width: 30px; flex-shrink: 0;} #b {width: 30px; flex-shrink: 0;}",
    );
    assert_eq!(rect(&page, "a").x, 0);
    assert_eq!(rect(&page, "b").x, 30);
    let document = html::parse(PAIR).unwrap();
    let sheet = Sheet::parse("#parent {display: flex; width: 20px; justify-content: center;} #a {width: 30px; flex-shrink: 0;}").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    assert!(
        Layout::build(
            &input,
            Constraints::viewport(Extent {
                width: 320,
                height: 200
            }),
            &Text
        )
        .is_err()
    );
}
