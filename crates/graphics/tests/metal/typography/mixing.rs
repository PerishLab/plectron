use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Face, Output, Queue, Weight};
use std::time::Duration;
use webengine::Page;

#[test]
fn multilingual() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let output = Output::new(extent, 2.0).unwrap();
    let mut page = Page::new("app://atlas/index.html", extent).unwrap();
    let fonts = page.fonts();
    page.load(&mut crate::Source::new("multilingual")).unwrap();
    let loaded = page.frame().unwrap();
    page.acknowledge(loaded.id()).unwrap();
    let registration = fonts.register("lato", Weight::Normal, Face::Toy).unwrap();
    page.render().unwrap();
    let original = page.frame().unwrap();
    let pixels = Cpu.submit(&original.prepare(output).unwrap()).unwrap();
    page.acknowledge(original.id()).unwrap();
    fonts.unregister(&registration).unwrap();
    page.render().unwrap();
    let changed = page.frame().unwrap();
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    for frame in [&changed, &original] {
        let expected = Cpu.submit(&frame.prepare(output).unwrap()).unwrap();
        if frame.id() == original.id() {
            assert_eq!(expected, pixels);
        } else {
            assert!(
                expected != pixels,
                "removing the overriding font must repaint"
            );
        }
        let ticket = queue.submit(target, frame).unwrap();
        queue.wait(ticket, Duration::from_secs(10)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
    }
    queue.close(target).unwrap();
}
