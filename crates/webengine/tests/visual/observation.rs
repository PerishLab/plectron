use graphics::shared::Extent;
use webengine::{
    Fault, Page,
    resource::{Provider, Response},
};

struct Observed;
impl Provider for Observed {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script(
                    "document, visual",
                    "let seen = 0; let coverage = ''; const host = document.getElementById('list'); const observer = visual.observe(host, (state) => { seen = seen + 1; coverage = state.leading + ':' + state.trailing + ':' + state.items + ':' + state.start + ':' + state.end + ':' + state.visual + ':' + state.inset; }, 16);",
                )
                .into_bytes(),
            });
        }
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<head><script src='./main.js'></script></head><body><div id='list' style='width: 64px; height: 64px; overflow: auto;'><div style='height: 40px;'>A</div><div style='height: 40px;'>B</div><div style='height: 40px;'>C</div></div></body>".to_vec(),
        })
    }
}
#[test]
fn observation() {
    let mut page = Page::new(
        "app://observe/index.html",
        Extent {
            width: 64,
            height: 64,
        },
    )
    .unwrap();
    page.load(&mut Observed).unwrap();
    assert_eq!(page.evaluate("seen;").unwrap(), "1");
    assert_eq!(page.evaluate("coverage;").unwrap(), "0:56:3:0:2:0:0");
    let snapshot = page.inspect();
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "list"))
        .unwrap();
    let boxspace = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .space;
    let space = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(boxspace) && mapping.range.is_some())
        .unwrap()
        .id;
    page.pan(snapshot.scene.unwrap(), space, 40).unwrap();
    assert_eq!(page.evaluate("seen;").unwrap(), "2");
    assert_eq!(page.evaluate("coverage;").unwrap(), "40:16:3:0:3:1:-40");
    page.evaluate("observer.dispose();").unwrap();
    let scene = page.inspect().scene.unwrap();
    page.pan(scene, space, -20).unwrap();
    assert_eq!(page.evaluate("seen;").unwrap(), "2");
}
