use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Output, Queue};
use std::time::Duration;
use webengine::Page;

#[test]
fn direction() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let output = Output::new(extent, 2.0).unwrap();
    let mut page = Page::new("app://direction/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("direction")).unwrap();
    let original = page.frame().unwrap();
    page.acknowledge(original.id()).unwrap();
    page.evaluate("mixed.setAttribute('dir', 'ltr'); const editor = document.getElementById('editor'); editor.focus(); editor.setSelection(2, 7);").unwrap();
    page.render().unwrap();
    let changed = page.frame().unwrap();
    page.acknowledge(changed.id()).unwrap();
    page.evaluate(
        "mixed.setAttribute('dir', 'rtl'); editor.setSelection(0, 0); editor.setValue('');",
    )
    .unwrap();
    page.render().unwrap();
    let empty = page.frame().unwrap();
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    let mut previous = None;
    for frame in [&original, &changed, &empty, &original] {
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
