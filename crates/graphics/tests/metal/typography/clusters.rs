use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Output, Queue};
use std::time::Duration;
use webengine::Page;

#[test]
fn atlas() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let output = Output::new(extent, 2.0).unwrap();
    let mut page = Page::new("app://atlas/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("clusters")).unwrap();
    let original = page.frame().unwrap();
    let pixels = Cpu.submit(&original.prepare(output).unwrap()).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    page.acknowledge(original.id()).unwrap();
    page.evaluate(&fixtures::script(
        "document",
        "document.getElementById('combining').setText('Å é ffi');",
    ))
    .unwrap();
    page.render().unwrap();
    let changed = page.frame().unwrap();
    page.close();
    for frame in [&changed, &original] {
        let expected = Cpu.submit(&frame.prepare(output).unwrap()).unwrap();
        if frame.id() == original.id() {
            assert_eq!(expected, pixels);
        } else {
            assert_ne!(expected, pixels);
        }
        let ticket = queue.submit(target, frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
    }
    queue.close(target).unwrap();
}
