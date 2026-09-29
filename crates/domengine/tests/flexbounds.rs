use ::fixtures::Text;
use domengine::{Constraints, Document, Extent, Input, Layout, Sheet, html};

fn build(document: &Document, sheet: &Sheet) -> Result<Layout, domengine::Fault> {
    Layout::build(
        &Input::capture(document, sheet)?,
        Constraints::viewport(Extent {
            width: 300,
            height: 200,
        }),
        &Text,
    )
}
#[test]
fn automatic() {
    let document = html::parse("<div id='parent'><div id='child'></div></div>").unwrap();
    let sheet = Sheet::parse("#parent {display: flex; flex-direction: column; min-height: 100px;} #child {height: 20px; flex-grow: 1;}").unwrap();
    let layout = build(&document, &sheet).unwrap();
    let id = document.find("child").unwrap().id();
    assert_eq!(
        layout
            .fragments()
            .iter()
            .find(|fragment| fragment.node == Some(id))
            .unwrap()
            .rect
            .height,
        100
    );
    let document = html::parse("<div id='parent'><p id='child'>AB</p></div>").unwrap();
    let sheet = Sheet::parse(
        "#parent {display: flex; flex-direction: column; width: 8px; align-items: flex-start; overflow-wrap: anywhere;}",
    )
    .unwrap();
    let layout = build(&document, &sheet).unwrap();
    let id = document.find("child").unwrap().id();
    assert_eq!(
        layout
            .fragments()
            .iter()
            .find(|fragment| fragment.node == Some(id))
            .unwrap()
            .rect
            .width,
        16
    );
}
#[test]
fn anonymous() {
    let document = html::parse("<div id='parent'>AB<div id='child'>CD</div></div>").unwrap();
    let sheet = Sheet::parse("#parent {display: flex; width: 64px;}").unwrap();
    let layout = build(&document, &sheet).unwrap();
    assert_eq!(layout.hit(4, 4), document.find("parent"));
    assert_eq!(layout.hit(36, 4), document.find("child"));
    let id = document.find("child").unwrap().id();
    assert_eq!(
        layout
            .fragments()
            .iter()
            .find(|fragment| fragment.node == Some(id))
            .unwrap()
            .rect
            .x,
        32
    );
}
#[test]
fn overflow() {
    let document = html::parse("<div id='parent'><p id='a'>AB</p><p id='b'>CD</p></div>").unwrap();
    let sheet =
        Sheet::parse("#parent {display: flex; width: 8px;} p {overflow: hidden; height: 16px;}")
            .unwrap();
    let layout = build(&document, &sheet).unwrap();
    for name in ["a", "b"] {
        let id = document.find(name).unwrap().id();
        assert_eq!(
            layout
                .fragments()
                .iter()
                .find(|fragment| fragment.node == Some(id))
                .unwrap()
                .rect
                .width,
            4
        );
    }
    let document =
        html::parse("<div id='parent'><div id='a'></div><div id='b'></div></div>").unwrap();
    let sheet = Sheet::parse(
        "#parent {display: flex; width: 2px; gap: 4px;} #a {padding: 3px;} #b {padding: 3px;}",
    )
    .unwrap();
    let layout = build(&document, &sheet).unwrap();
    let id = document.find("b").unwrap().id();
    let rect = layout
        .fragments()
        .iter()
        .find(|fragment| fragment.node == Some(id))
        .unwrap()
        .rect;
    assert_eq!((rect.x, rect.width), (10, 6));
}
#[test]
fn limits() {
    let document = html::parse(&format!(
        "<div id='parent'>{}</div>",
        "<div></div>".repeat(257)
    ))
    .unwrap();
    let sheet = Sheet::parse("#parent {display: flex;}").unwrap();
    assert!(
        build(&document, &sheet)
            .unwrap_err()
            .0
            .contains("flex item budget")
    );
    let config = config::Store::default();
    config.set("layout.items", 512.into());
    config.set("layout.admission", 65_536.into());
    let mut pipeline =
        domengine::Pipeline::configured(Text, &shared::cache::Manager::default(), config.clone())
            .unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let constraints = Constraints::viewport(Extent {
        width: 300,
        height: 200,
    });
    pipeline.build(&input, constraints).unwrap();
    pipeline.build(&input, constraints).unwrap();
    assert_eq!(pipeline.inspect().retained, 1);
    assert_eq!(pipeline.inspect().hits, 0);
    config.set("layout.items", 256.into());
    assert!(
        pipeline
            .build(&input, constraints)
            .unwrap_err()
            .0
            .contains("flex item budget")
    );
    config.set("layout.items", 257.into());
    pipeline.build(&input, constraints).unwrap();
    config.set("layout.items", (-1).into());
    assert!(
        pipeline
            .build(&input, constraints)
            .unwrap_err()
            .0
            .contains("config layout.items")
    );
}

#[test]
fn scrolling() {
    let document = html::parse(
        "<div id='parent'><div id='rows'><div id='a'></div><div id='b'></div></div></div>",
    )
    .unwrap();
    let sheet = Sheet::parse("#parent {display: flex; flex-direction: column; height: 60px;} #rows {display: flex; flex-direction: column; flex-grow: 1; min-height: 0; overflow: auto; gap: 10px;} #a {height: 50px; flex-shrink: 0;} #b {height: 50px; flex-shrink: 0;}").unwrap();
    let layout = build(&document, &sheet).unwrap();
    let id = document.find("rows").unwrap().id();
    let rows = layout
        .fragments()
        .iter()
        .find(|fragment| fragment.node == Some(id))
        .unwrap();
    assert_eq!(rows.rect.height, 60);
    assert!(matches!(
        rows.content,
        domengine::Content::Box {
            scroll: Some(50),
            ..
        }
    ));
    let document = html::parse("<div style='overflow: auto;'>AB</div>").unwrap();
    assert!(
        build(&document, &Sheet::default())
            .unwrap_err()
            .0
            .contains("definite height")
    );
}

struct Counting(std::cell::Cell<usize>);
impl domengine::Shaper for Counting {
    fn revision(&self) -> u64 {
        1
    }
    fn select(
        &self,
        source: &str,
        request: &textengine::Request,
        spacing: usize,
    ) -> Result<Vec<textengine::Selection>, textengine::Fault> {
        self.0.set(self.0.get() + 1);
        domengine::Shaper::select(&Text, source, request, spacing)
    }
}

#[test]
fn basis() {
    let document = html::parse("<div><p id='item'>AB</p></div>").unwrap();
    let sheet = Sheet::parse("div {display: flex; width: 100px; align-items: flex-start;} p {flex-basis: 0px; flex-grow: 1; min-width: 0px;}").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let shaper = Counting(std::cell::Cell::new(0));
    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 300,
            height: 200,
        }),
        &shaper,
    )
    .unwrap();
    assert_eq!(shaper.0.get(), 1);
    let node = document.find("item").unwrap();
    let fragment = layout
        .fragments()
        .iter()
        .find(|fragment| fragment.node == Some(node.id()))
        .unwrap();
    assert_eq!((fragment.rect.width, fragment.rect.height), (100, 16));
}
