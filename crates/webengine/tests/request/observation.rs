use super::{Page, until};

pub(super) fn complete(page: &mut Page) {
    let session = observe::Session::new(Default::default());
    let mut source = session.scope("request.source");
    page.evaluate("let observed = ''; const observation = async () => { const reply = await fetch('/missing'); observed = await reply.text(); }; observation();").unwrap();
    source.finish(true);
    let foreign = observe::Session::new(Default::default());
    let mut poll = foreign.scope("unrelated.poll");
    until(|| {
        page.poll().unwrap();
        page.evaluate("observed;").unwrap() == "hello"
    });
    poll.finish(true);
    until(|| session.status().open == 0);
    session.stop();
    foreign.stop();
    let report = session.inspect();
    let transport = report
        .records
        .iter()
        .find(|r| r.name == "resource.transport")
        .unwrap();
    let completion = report
        .records
        .iter()
        .find(|r| r.name == "resource.complete")
        .unwrap();
    assert_eq!(completion.parent, Some(transport.id));
    assert_ne!(transport.thread, completion.thread);
    assert!(
        report
            .records
            .iter()
            .any(|r| r.name == "js.job" && descendant(&report, r, completion.id))
    );
    assert!(
        foreign
            .inspect()
            .records
            .iter()
            .all(|r| r.name != "js.job" && r.name != "resource.complete")
    );
    assert_eq!(report.dropped, 0);
}

fn descendant(report: &observe::Report, record: &observe::Record, id: u64) -> bool {
    let mut parent = record.parent;
    while let Some(current) = parent {
        if current == id {
            return true;
        }
        parent = report
            .records
            .iter()
            .find(|r| r.id == current)
            .and_then(|r| r.parent);
    }
    false
}
