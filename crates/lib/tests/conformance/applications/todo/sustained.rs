use super::{evaluate, rows, setup, tap};
use crate::web::{call, snapshot};
use plectron::ipc::{Command, Reply};

fn collect(
    session: &mut plectron::ipc::Session<crate::device::Device, plectron::rendering::Solid>,
) -> std::collections::BTreeMap<String, usize> {
    let Reply::Evaluation { display } = call(session, Command::Collect { unit: 1 }) else {
        panic!()
    };
    display
        .split_whitespace()
        .map(|field| field.split_once('=').unwrap())
        .map(|(name, value)| (name.into(), value.parse().unwrap()))
        .collect()
}

fn cycle(
    session: &mut plectron::ipc::Session<crate::device::Device, plectron::rendering::Solid>,
    device: &crate::device::Device,
) {
    evaluate(
        session,
        "document.getElementById('task-input').setValue('Repeated task');",
    );
    tap(session, device, "add");
    let current = snapshot(session);
    let remove = current
        .nodes
        .iter()
        .filter_map(|node| node.attributes.get("id"))
        .find(|id| id.starts_with("remove-"))
        .unwrap()
        .clone();
    drop(current);
    tap(session, device, &remove);
    assert_eq!(rows(&snapshot(session)), 3);
}

fn footprint(report: &std::collections::BTreeMap<String, usize>) -> [usize; 7] {
    [
        report["js_capacity"],
        report["js_live"],
        report["js_vacant"],
        report["js_edges"],
        report["dom_capacity"],
        report["dom_live"],
        report["dom_vacant"],
    ]
}

#[test]
fn retention() {
    let (mut session, device) = setup();
    let observation = observe::Session::new(Default::default());
    let mut interval = observation.scope("todo.retention");
    let mut retained = None;
    for _ in 0..16 {
        cycle(&mut session, &device);
        let report = collect(&mut session);
        assert_eq!(
            report["js_live"] + report["js_vacant"],
            report["js_capacity"]
        );
        assert_eq!(
            report["dom_live"] + report["dom_vacant"],
            report["dom_capacity"]
        );
        let current = footprint(&report);
        if let Some(retained) = retained {
            assert_eq!(current, retained);
        } else {
            retained = Some(current);
        }
    }
    interval.finish(true);
    observation.stop();
    let report = observation.inspect();
    let javascript = report
        .records
        .iter()
        .filter(|record| record.name == "js.collect")
        .count();
    let document = report
        .records
        .iter()
        .filter(|record| record.name == "dom.collect")
        .count();
    assert!(javascript >= 16);
    assert_eq!(javascript, document);
    assert_eq!(report.dropped, 0);
    assert!(
        report
            .records
            .iter()
            .filter(|record| record.name == "js.collect" || record.name == "dom.collect")
            .all(|record| record.outcome == observe::Outcome::Success)
    );
}

#[test]
fn automatic() {
    let (mut session, device) = setup();
    cycle(&mut session, &device);
    let retained = footprint(&collect(&mut session));
    let observation = observe::Session::new(Default::default());
    let mut interval = observation.scope("todo.automatic");
    for _ in 0..16 {
        cycle(&mut session, &device);
    }
    interval.finish(true);
    observation.stop();
    let report = observation.inspect();
    let causes: std::collections::BTreeSet<_> = report
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.name.as_str(),
                "web.collect.objects" | "web.collect.nodes" | "web.collect.both"
            )
        })
        .map(|record| record.id)
        .collect();
    assert!(!causes.is_empty());
    assert!(
        report
            .records
            .iter()
            .all(|record| record.name != "web.collect.manual")
    );
    let nested: Vec<_> = report
        .records
        .iter()
        .filter(|record| record.name == "js.collect" || record.name == "dom.collect")
        .collect();
    assert_eq!(nested.len(), causes.len() * 2);
    assert!(
        nested
            .iter()
            .all(|record| record.parent.is_some_and(|parent| causes.contains(&parent)))
    );
    assert_eq!(report.dropped, 0);
    let report = collect(&mut session);
    let current = footprint(&report);
    assert_eq!(&current[0..4], &retained[0..4]);
    assert_eq!(current[5], retained[5]);
    assert_eq!(
        report["dom_live"] + report["dom_vacant"],
        report["dom_capacity"]
    );
    for _ in 0..16 {
        cycle(&mut session, &device);
    }
    assert_eq!(footprint(&collect(&mut session)), current);
}
