use graphics::shared::Extent;
use webengine::{
    Fault, Page,
    resource::{Provider, Response},
};

#[derive(Clone)]
struct Application(fixtures::Application);
impl Provider for Application {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        let resource = self.0.load(request.url.path()).map_err(Fault)?;
        request.accept(Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
}

fn indices(page: &mut Page) -> Vec<usize> {
    page.evaluate("scenario.range();")
        .unwrap()
        .split(':')
        .map(|value| value.parse::<usize>().unwrap())
        .collect()
}

fn space(page: &mut Page) -> (u64, u64) {
    let snapshot = page.inspect();
    let node = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id == "visual-window")
        })
        .unwrap();
    let fragment = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap();
    let mapping = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(fragment.space) && mapping.range.is_some())
        .unwrap();
    (snapshot.scene.unwrap(), mapping.id)
}

#[test]
fn logicalwindow() {
    let mut page = Page::new(
        "app://visual/index.html",
        Extent {
            width: 720,
            height: 640,
        },
    )
    .unwrap();
    page.load(&mut Application(fixtures::Application::new("visual")))
        .unwrap();
    assert_eq!(page.evaluate("scenario.count();").unwrap(), "100000");
    let live = page
        .inspect()
        .nodes
        .iter()
        .filter(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id.starts_with("visual-record-"))
        })
        .count();
    assert!((1..64).contains(&live));
    let range = indices(&mut page);
    assert_eq!(range[1], 100000);
    assert!(range[1] - range[0] < 64);
    assert_eq!(range[2], 100000);
    let (_, tail) = space(&mut page);
    assert!(page.inspect().visual.offset(tail) > 0);
    page.evaluate("scenario.jump(50000);").unwrap();
    page.render().unwrap();
    let range = indices(&mut page);
    assert!(range[0] <= 50000 && 50000 < range[1]);
    assert!(range[1] - range[0] < 64);
    let (_, space) = space(&mut page);
    assert!(page.inspect().visual.offset(space) > 0);
}

#[test]
fn scrolling() {
    let mut page = Page::new(
        "app://visual/index.html",
        Extent {
            width: 720,
            height: 640,
        },
    )
    .unwrap();
    page.load(&mut Application(fixtures::Application::new("visual")))
        .unwrap();
    page.evaluate("scenario.start();").unwrap();
    page.render().unwrap();
    assert_eq!(indices(&mut page)[0], 0);
    let (_, start) = space(&mut page);
    assert!(page.inspect().visual.offset(start) <= 12);
    for _ in 0..24 {
        let (scene, space) = space(&mut page);
        page.pan(scene, space, 160).unwrap();
    }
    let range = indices(&mut page);
    assert!(range[0] > 0);
    assert!(range[1] - range[0] < 64);
    let live = page
        .inspect()
        .nodes
        .iter()
        .filter(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id.starts_with("visual-record-"))
        })
        .count();
    assert!(live < 64);
    for _ in 0..24 {
        let (scene, space) = space(&mut page);
        page.pan(scene, space, -160).unwrap();
    }
    let range = indices(&mut page);
    assert_eq!(range[0], 0);
    assert!(range[1] - range[0] < 64);
}

#[test]
fn resizing() {
    let mut page = Page::new(
        "app://visual/index.html",
        Extent {
            width: 720,
            height: 640,
        },
    )
    .unwrap();
    page.load(&mut Application(fixtures::Application::new("visual")))
        .unwrap();
    page.evaluate("scenario.jump(50001);").unwrap();
    page.render().unwrap();
    let (_, space) = space(&mut page);
    let offset = page.inspect().visual.offset(space);
    let before = page.inspect().revision;
    page.evaluate("scenario.grow(50001);").unwrap();
    page.render().unwrap();
    let after = page.inspect();
    assert!(after.revision > before);
    assert_eq!(after.visual.offset(space), offset);
    assert!(after.nodes.iter().any(|node| {
        node.attributes
            .get("id")
            .is_some_and(|id| id == "visual-record-50001")
            && node
                .attributes
                .get("class")
                .is_some_and(|class| class == "item tall")
    }));
}
