#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Queue};
use std::time::{Duration, Instant};
use webengine::Page;
#[test]
fn example() {
    let extent = Extent {
        width: 640,
        height: 420,
    };
    let mut page = Page::new("app://request/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("request")).unwrap();
    let first = page.frame().unwrap();
    page.acknowledge(first.id()).unwrap();
    page.evaluate("document.getElementById('load').dispatchEvent('click', '');")
        .unwrap();
    page.render().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        page.poll().unwrap();
        if page
            .inspect()
            .nodes
            .iter()
            .any(|node| node.content == "HTTP 200")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let frame = page.frame().unwrap();
    assert_ne!(Cpu.render(&first).unwrap(), Cpu.render(&frame).unwrap());
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    for frame in [first, frame] {
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(3)).unwrap();
        assert_eq!(queue.read(ticket).unwrap(), Cpu.render(&frame).unwrap());
        queue.release(ticket).unwrap();
    }
    page.close();
    queue.close(target).unwrap();
}
