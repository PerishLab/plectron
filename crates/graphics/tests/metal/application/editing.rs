#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Frame, Image, Queue, Target};
use std::time::Duration;
use webengine::{Page, Snapshot};

fn frame(page: &mut Page, queue: &mut Queue<Metal>, target: Target) -> (Frame, Image) {
    let frame = page.frame().unwrap();
    let ticket = queue.submit(target, &frame).unwrap();
    queue.wait(ticket, Duration::from_secs(5)).unwrap();
    let image = queue.read(ticket).unwrap();
    assert_eq!(image, Cpu.render(&frame).unwrap());
    page.acknowledge(frame.id()).unwrap();
    queue.release(ticket).unwrap();
    (frame, image)
}
fn rectangle(snapshot: &Snapshot, name: &str) -> graphics::shared::Rect {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == name))
        .unwrap();
    snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .rect
}
#[test]
fn interaction() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let mut page = Page::new("app://tasks/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("list")).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    let (original, initial) = frame(&mut page, &mut queue, target);
    page.edit(&webengine::Edit::Insert("long title ".repeat(10)))
        .unwrap();
    page.edit(&webengine::Edit::All).unwrap();
    page.render().unwrap();
    let (selected, highlighted) = frame(&mut page, &mut queue, target);
    assert!(highlighted.pixels.contains(&0x9fc5f8));
    page.edit(&webengine::Edit::Select(webengine::Selection {
        anchor: 110,
        focus: 0,
    }))
    .unwrap();
    page.render().unwrap();
    let (reversed, left) = frame(&mut page, &mut queue, target);
    assert!(left.pixels.contains(&0x9fc5f8));
    assert_ne!(selected.id(), reversed.id());
    assert_ne!(left, highlighted);
    page.edit(&webengine::Edit::Insert("Build a modern browser".into()))
        .unwrap();
    page.render().unwrap();
    let (_, edited) = frame(&mut page, &mut queue, target);
    assert_ne!(edited, initial);
    assert_eq!(
        page.evaluate("title.getValue();").unwrap(),
        "Build a modern browser"
    );
    page.edit(&webengine::Edit::Commit).unwrap();
    page.render().unwrap();
    let (_, added) = frame(&mut page, &mut queue, target);
    assert_ne!(added, edited);
    assert_eq!(
        page.evaluate("items[3].title;").unwrap(),
        "Build a modern browser"
    );
    let first = rectangle(&page.inspect(), "task-0");
    let hit = page.hit(first.x + 4, first.y + 4);
    let scene = page.inspect().scene;
    page.wheel(first.x + 4, first.y + 4, 100).unwrap();
    let (scrolled, pixels) = frame(&mut page, &mut queue, target);
    assert_eq!(Some(scrolled.scene().id()), scene);
    assert_ne!(pixels, added);
    page.dispatch(&hit).unwrap();
    page.render().unwrap();
    frame(&mut page, &mut queue, target);
    assert_eq!(page.evaluate("items[0].done;").unwrap(), "true");
    page.close();
    let ticket = queue.submit(target, &original).unwrap();
    queue.wait(ticket, Duration::from_secs(5)).unwrap();
    assert_eq!(queue.read(ticket).unwrap(), initial);
    queue.release(ticket).unwrap();
    queue.close(target).unwrap();
}
