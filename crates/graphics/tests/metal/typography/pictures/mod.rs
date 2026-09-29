mod residency;
use super::compare;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Frame, Output, Queue};
use std::{sync::Arc, time::Duration};
use webengine::{
    Fault, Page,
    resource::{Cancellation, Provider, Request, Response, Transport, Url},
};
#[derive(Clone)]
struct Source {
    pictures: crate::Source,
    atoms: crate::Source,
}
impl Source {
    fn new() -> Self {
        Self {
            pictures: crate::Source::new("pictures"),
            atoms: crate::Source::new("atoms"),
        }
    }
    fn select(&self, url: &Url) -> &crate::Source {
        if url.host_str() == Some("atoms") {
            &self.atoms
        } else {
            &self.pictures
        }
    }
}
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        let mut source = self.select(&request.url).clone();
        source.request(request)
    }
    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(Arc::new(self.clone()))
    }
}
impl Transport for Source {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        self.select(&request.url).send(request, cancellation)
    }
}
#[test]
fn composition() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let mut page = Page::new("app://pictures/index.html", extent).unwrap();
    page.load(&mut Source::new()).unwrap();
    let original = page.frame().unwrap();
    page.acknowledge(original.id()).unwrap();
    let colored = click(&mut page, "landscape");
    assert_eq!(page.evaluate("dusk;").unwrap(), "true");
    click(&mut page, "size");
    let changed = click(&mut page, "fade");
    assert_eq!(page.evaluate("small && faded;").unwrap(), "true");
    assert_eq!(
        page.inspect()
            .resources
            .iter()
            .filter(|resource| resource.mime == "image/png")
            .count(),
        3
    );
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    for scale in [1.0, 1.5, 2.0] {
        let output = Output::new(extent, scale).unwrap();
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
}
fn click(page: &mut Page, id: &str) -> Frame {
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

#[test]
fn inline() {
    let extent = Extent {
        width: 720,
        height: 700,
    };
    let mut page = Page::new("app://atoms/index.html", extent).unwrap();
    page.load(&mut Source::new()).unwrap();
    let original = page.frame().unwrap();
    page.acknowledge(original.id()).unwrap();
    let enlarged = click(&mut page, "icon");
    assert_eq!(page.evaluate("large;").unwrap(), "true");
    let narrow = click(&mut page, "measure");
    assert_eq!(page.evaluate("narrow;").unwrap(), "true");
    let dusk = click(&mut page, "swap");
    assert_eq!(page.evaluate("dusk;").unwrap(), "true");
    click(&mut page, "measure");
    let restored = click(&mut page, "icon");
    assert_eq!(page.evaluate("!large && !narrow;").unwrap(), "true");
    page.close();
    let mut queue = Queue::new(Metal::new().unwrap());
    for scale in [1.0, 2.0] {
        let output = Output::new(extent, scale).unwrap();
        let target = queue.output(output).unwrap();
        let mut previous = None;
        for frame in [&restored, &dusk, &narrow, &enlarged, &original] {
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
}
