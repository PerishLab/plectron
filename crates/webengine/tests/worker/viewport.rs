use super::*;
use webengine::Page;

#[test]
fn ownership() {
    let _guard = serial();
    local();
    threaded();
}

fn local() {
    let original = Extent {
        width: 64,
        height: 32,
    };
    let resized = Extent {
        width: 120,
        height: 96,
    };
    let mut page = Page::new("app://worker/index.html", original).unwrap();
    page.load(&mut Source).unwrap();
    page.evaluate("let retained = 7; document.getElementById('a').setText('retained');")
        .unwrap();
    page.render().unwrap();
    let initial = page.frame().unwrap();
    page.acknowledge(initial.id()).unwrap();
    let before = page.inspect();
    page.resize(resized).unwrap();
    assert_eq!(page.inspect().visual.extent, resized);
    assert_eq!(page.evaluate("retained;").unwrap(), "7");
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "retained")
    );
    assert_eq!(page.hit(8, 8).frame(), initial.id());
    let frame = page.frame().unwrap();
    assert_eq!(frame.extent(), resized);
    assert_ne!(frame.scene().id(), initial.scene().id());
    assert_eq!(page.inspect().revision, before.revision);
    page.acknowledge(frame.id()).unwrap();
    assert_eq!(page.hit(8, 8).frame(), frame.id());
    assert!(
        page.resize(Extent {
            width: 0,
            height: 32
        })
        .is_err()
    );
    assert_eq!(page.inspect().visual.extent, resized);
    page.close();
    assert!(page.resize(original).is_err());
}

fn threaded() {
    let mut worker = ready();
    evaluate(&mut worker, "let retained = 9;");
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    let extent = Extent {
        width: 120,
        height: 96,
    };
    let resize = worker.submit(Action::Resize(extent)).unwrap();
    worker.scroll(16).unwrap();
    worker.poll().unwrap();
    assert_eq!(
        worker.inspect().visual.extent,
        Extent {
            width: 64,
            height: 32
        }
    );
    worker.unhold().unwrap();
    until(&mut worker, |snapshot| {
        phase(snapshot, resize) == Phase::Complete
    });
    assert_eq!(worker.inspect().visual.extent, extent);
    let frame = worker.frame().unwrap();
    assert_eq!(frame.extent(), extent);
    worker.acknowledge(frame.id()).unwrap();
    let value = evaluate(&mut worker, "retained;");
    assert_eq!(
        worker.job(value).unwrap().result().cloned(),
        Some(Ok("9".into()))
    );
    assert!(worker.errors().is_empty());
    worker.close();
    assert!(worker.submit(Action::Resize(extent)).is_err());
}
