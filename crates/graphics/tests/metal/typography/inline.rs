use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Output, Queue};
use std::time::Duration;
use webengine::Page;

#[test]
fn interaction() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let output = Output::new(extent, 2.0).unwrap();
    let mut page = Page::new("app://inline/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("inline")).unwrap();
    let original = page.frame().unwrap();
    page.acknowledge(original.id()).unwrap();
    click(&mut page, "ligature");
    assert_eq!(page.evaluate("selected;").unwrap(), "ligature");
    click(&mut page, "arabic");
    assert_eq!(page.evaluate("selected;").unwrap(), "arabic");
    let colored = click(&mut page, "tone");
    assert_eq!(page.evaluate("green;").unwrap(), "true");
    click(&mut page, "measure");
    let changed = click(&mut page, "tone");
    assert_eq!(page.evaluate("narrow && !green;").unwrap(), "true");
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    let mut previous = None;
    for frame in [&changed, &colored, &original] {
        let expected = Cpu.submit(&frame.prepare(output).unwrap()).unwrap();
        if let Some(previous) = previous {
            assert_ne!(expected, previous);
        }
        let ticket = queue.submit(target, frame).unwrap();
        queue.wait(ticket, Duration::from_secs(10)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
        previous = Some(expected);
    }
    queue.close(target).unwrap();
}
fn click(page: &mut Page, id: &str) -> graphics::shared::Frame {
    let snapshot = page.inspect();
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").map(String::as_str) == Some(id))
        .unwrap();
    let leaf = snapshot
        .nodes
        .iter()
        .find(|child| child.parent == Some(node.id))
        .unwrap();
    let region = snapshot
        .visible
        .iter()
        .filter(|region| region.node == Some(leaf.id))
        .max_by_key(|region| region.rect.width)
        .unwrap();
    page.click(
        region.rect.x + region.rect.width / 2,
        region.rect.y + region.rect.height / 2,
    )
    .unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    frame
}
