use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Output, Queue};
use std::time::Duration;
use webengine::Page;

#[test]
fn reshaping() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let output = Output::new(extent, 2.0).unwrap();
    let mut page = Page::new("app://reshaping/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("reshaping")).unwrap();
    let original = page.frame().unwrap();
    page.acknowledge(original.id()).unwrap();
    click(&mut page, "word");
    click(&mut page, "width");
    assert_eq!(
        page.evaluate("alternate === true && narrow === false;")
            .unwrap(),
        "true"
    );
    assert_eq!(
        page.inspect()
            .nodes
            .iter()
            .filter(|node| node.content == "بلا")
            .count(),
        2
    );
    page.render().unwrap();
    let changed = page.frame().unwrap();
    page.acknowledge(changed.id()).unwrap();
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    let mut previous = None;
    for frame in [&changed, &original] {
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

fn click(page: &mut Page, id: &str) {
    let snapshot = page.inspect();
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").map(String::as_str) == Some(id))
        .unwrap();
    let region = snapshot
        .visible
        .iter()
        .find(|region| region.node == Some(node.id))
        .unwrap();
    page.click(region.rect.x + 1, region.rect.y + 1).unwrap();
}
