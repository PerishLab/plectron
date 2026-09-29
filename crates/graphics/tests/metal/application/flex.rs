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
fn reflow() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let mut page = Page::new("app://flex/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("flex")).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    let (original, pixels) = frame(&mut page, &mut queue, target);
    let before = rectangle(&page.inspect(), "panel-1");
    let row = rectangle(&page.inspect(), "row-1-0");
    let old = page.hit(row.x + 4, row.y + 4);
    assert_eq!(pixels.pixels[(row.y + 40) * 720 + row.x + 20], 0x334155);
    page.evaluate("add();").unwrap();
    page.render().unwrap();
    let (_, added) = frame(&mut page, &mut queue, target);
    assert!(rectangle(&page.inspect(), "panel-1").width < before.width);
    assert_ne!(added, pixels);
    page.dispatch(&old).unwrap();
    assert_eq!(page.evaluate("selected;").unwrap(), "1:0");
    page.render().unwrap();
    let (_, selected) = frame(&mut page, &mut queue, target);
    let row = rectangle(&page.inspect(), "row-1-0");
    assert_eq!(selected.pixels[(row.y + 40) * 720 + row.x + 20], 0x0e7490);
    assert_eq!(page.messages(), ["selected:1:0"]);
    let scene = page.inspect().scene;
    let row = rectangle(&page.inspect(), "row-0-0");
    page.wheel(row.x + 4, row.y + 4, 72).unwrap();
    let (scrolled, _) = frame(&mut page, &mut queue, target);
    assert_eq!(Some(scrolled.scene().id()), scene);
    page.dispatch(&page.hit(row.x + 4, row.y + 4)).unwrap();
    assert_eq!(page.evaluate("selected;").unwrap(), "0:1");
    page.evaluate("remove();").unwrap();
    page.render().unwrap();
    let (reduced, _) = frame(&mut page, &mut queue, target);
    assert_eq!(rectangle(&page.inspect(), "panel-1").width, before.width);
    assert_ne!(reduced.scene().id(), scrolled.scene().id());
    page.close();
    let ticket = queue.submit(target, &original).unwrap();
    queue.wait(ticket, Duration::from_secs(5)).unwrap();
    assert_eq!(queue.read(ticket).unwrap(), pixels);
    queue.release(ticket).unwrap();
    queue.close(target).unwrap();
}
