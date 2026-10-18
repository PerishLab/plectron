use crate::device::{self, Device};
use crate::web::{call, snapshot};
use plectron::addons::control::Pump;
use plectron::ipc::{Command, Reply, Session};
use plectron::platform::{
    Host,
    device::{Event, Point, Signal},
};
use plectron::rendering::Solid;
use webengine::{Snapshot, State};

mod channel;

fn messages(snapshot: &Snapshot) -> usize {
    snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id.starts_with("chat-message-"))
        })
        .count()
}

fn scrollspace(snapshot: &Snapshot, id: &str) -> (u64, usize, Point) {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap();
    let boxspace = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .space;
    let mapping = snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(boxspace) && mapping.range.is_some())
        .unwrap();
    (
        mapping.id,
        mapping.range.unwrap(),
        Point {
            x: mapping.clip.x as i64 + 4,
            y: mapping.clip.y as i64 + 4,
        },
    )
}

fn wheel(session: &mut Session<Device, Solid>, device: &Device, point: Point, delta: i64) {
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Wheel {
            position: point,
            delta,
        },
    });
    session.tick().unwrap();
}

fn commit(session: &mut Session<Device, Solid>, text: &str) {
    call(
        session,
        Command::Edit {
            unit: 1,
            edit: webengine::Edit::Insert(text.into()),
        },
    );
    session.tick().unwrap();
    call(
        session,
        Command::Edit {
            unit: 1,
            edit: webengine::Edit::Commit,
        },
    );
    session.tick().unwrap();
}

fn evaluate(session: &mut Session<Device, Solid>, source: &str) -> String {
    let Reply::Evaluation { display } = call(
        session,
        Command::Evaluate {
            unit: 1,
            source: source.into(),
        },
    ) else {
        panic!()
    };
    display
}

#[test]
fn composing() {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/chat/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let first = snapshot(&mut session);
    assert_eq!(first.state, State::Ready, "{:?}", first.fault);
    assert_eq!(messages(&first), 32);
    assert_eq!(first.relevance.overscan, 400);
    assert_eq!(first.relevance.boundaries, messages(&first) + 1);
    assert!(first.relevance.relevant < first.relevance.boundaries);
    assert!(first.relevance.pending > 0);
    assert_eq!(
        evaluate(
            &mut session,
            "document.focused() === document.getElementById('message-input');"
        ),
        "true"
    );
    let (timeline, initial_range, _) = scrollspace(&first, "timeline");
    assert!(initial_range > 0);
    assert_eq!(first.visual.offset(timeline), initial_range);

    for text in [
        "Plectron ",
        "中文输入",
        " keeps a long bilingual message anchored while incremental layout wraps it across visual lines.",
    ] {
        call(
            &mut session,
            Command::Edit {
                unit: 1,
                edit: webengine::Edit::Insert(text.into()),
            },
        );
        session.tick().unwrap();
    }
    call(
        &mut session,
        Command::Edit {
            unit: 1,
            edit: webengine::Edit::Commit,
        },
    );
    session.tick().unwrap();

    let sent = snapshot(&mut session);
    assert_eq!(messages(&sent), 33);
    assert!(sent.update.affected < sent.update.nodes);
    assert!(sent.update.rebuilt < sent.update.nodes);
    assert!(sent.reuse.hits > sent.reuse.computed, "{:?}", sent.reuse);
    assert!(
        sent.painting.hits > sent.painting.built,
        "{:?}",
        sent.painting
    );
    let (grown_timeline, grown_range, _) = scrollspace(&sent, "timeline");
    assert_eq!(grown_timeline, timeline);
    assert!(grown_range > initial_range);
    assert_eq!(sent.visual.offset(timeline), grown_range);
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('message-input').getValue();"
        ),
        ""
    );
    assert!(sent.nodes.iter().any(|node| {
        node.attributes
            .get("id")
            .is_some_and(|id| id == "chat-message-33")
    }));
    assert!(
        sent.nodes
            .iter()
            .any(|node| node.content.starts_with("Plectron 中文输入 keeps"))
    );
    for text in ["Batch one", "批量消息二", "Batch three"] {
        commit(&mut session, text);
    }
    let batched = snapshot(&mut session);
    assert_eq!(messages(&batched), 36);
    let (_, batch_range, batch_point) = scrollspace(&batched, "timeline");
    assert!(batch_range > grown_range);
    assert_eq!(batched.visual.offset(timeline), batch_range);

    wheel(&mut session, &device, batch_point, -40);
    let history = snapshot(&mut session).visual.offset(timeline);
    assert_eq!(history, batch_range - 40);
    evaluate(
        &mut session,
        "document.getElementById('history').dispatchEvent('click', '');",
    );
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('history').getAttribute('class');"
        ),
        "history historyloading"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let loaded = loop {
        session.tick().unwrap();
        let current = snapshot(&mut session);
        if messages(&current) == 100 {
            break current;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "history request did not converge: {} {:?} {:?}",
            messages(&current),
            current.requests,
            current.jobs
        );
    };
    assert_eq!(loaded.relevance.boundaries, messages(&loaded) + 1);
    assert!(loaded.relevance.relevant < loaded.relevance.boundaries);
    assert!(loaded.relevance.pending > 0);
    assert!(loaded.update.affected < loaded.update.nodes);
    assert!(
        loaded.reuse.hits > loaded.reuse.computed,
        "{:?}",
        loaded.reuse
    );
    let (_, loaded_range, _) = scrollspace(&loaded, "timeline");
    assert!(loaded_range > batch_range);
    assert_eq!(
        loaded.visual.offset(timeline),
        history + loaded_range - batch_range
    );
    assert_eq!(
        loaded.scrolling.last().unwrap().cause,
        webengine::ScrollCause::Anchor
    );
    let history = loaded.visual.offset(timeline);
    commit(&mut session, "Message while reading history");
    let preserved = snapshot(&mut session);
    assert_eq!(messages(&preserved), 101);
    assert!(scrollspace(&preserved, "timeline").1 > loaded_range);
    assert_eq!(preserved.visual.offset(timeline), history);
    let (_, preserved_range, _) = scrollspace(&preserved, "timeline");
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('latest').getAttribute('class');"
        ),
        "latest"
    );
    evaluate(
        &mut session,
        "document.getElementById('latest').dispatchEvent('click', '');",
    );
    let returned = snapshot(&mut session);
    assert_eq!(
        returned.visual.offset(timeline),
        scrollspace(&returned, "timeline").1
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let class = evaluate(
            &mut session,
            "document.getElementById('latest').getAttribute('class');",
        );
        if class == "latest latesthidden" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "latest state did not converge: {class}; {:?}",
            returned.scrolling
        );
        session.tick().unwrap();
    }
    commit(&mut session, "Following the end again");
    let resumed = snapshot(&mut session);
    assert_eq!(messages(&resumed), 102);
    let (_, resumed_range, _) = scrollspace(&resumed, "timeline");
    assert!(resumed_range > preserved_range);
    assert_eq!(resumed.visual.offset(timeline), resumed_range);
    evaluate(
        &mut session,
        "document.getElementById('history').dispatchEvent('click', ''); document.getElementById('history').dispatchEvent('click', ''); document.getElementById('engine-channel').dispatchEvent('click', '');",
    );
    session.tick().unwrap();
    let switched = snapshot(&mut session);
    assert_eq!(messages(&switched), 32);
    assert!(switched.requests.cancelled >= 2, "{:?}", switched.requests);
    evaluate(
        &mut session,
        "document.getElementById('general-channel').dispatchEvent('click', '');",
    );
    session.tick().unwrap();
    assert_eq!(messages(&snapshot(&mut session)), 102);
    call(&mut session, Command::Shutdown {});
    assert!(device.0.borrow().windows.is_empty());
}
