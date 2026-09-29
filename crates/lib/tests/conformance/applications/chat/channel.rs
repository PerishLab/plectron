use super::*;

fn value(session: &mut Session<Device, Solid>) -> String {
    evaluate(
        session,
        "document.getElementById('message-input').getValue();",
    )
}

fn insert(session: &mut Session<Device, Solid>, text: &str) {
    call(
        session,
        Command::Edit {
            unit: 1,
            edit: webengine::Edit::Insert(text.into()),
        },
    );
    session.tick().unwrap();
}

fn select(session: &mut Session<Device, Solid>, id: &str) {
    evaluate(
        session,
        &format!("document.getElementById('{id}').dispatchEvent('click', '');"),
    );
    session.tick().unwrap();
}

#[test]
fn ownership() {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/chat/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let initial = snapshot(&mut session);
    assert_eq!(initial.state, State::Ready, "{:?}", initial.fault);
    let (_, _, point) = scrollspace(&initial, "timeline");
    wheel(&mut session, &device, point, -80);
    let general = snapshot(&mut session);
    let (timeline, _, _) = scrollspace(&general, "timeline");
    let generalmark = evaluate(&mut session, "scenario.position();");
    assert_ne!(generalmark, "end");

    select(&mut session, "engine-channel");
    let engine = snapshot(&mut session);
    let (_, engine_range, engine_point) = scrollspace(&engine, "timeline");
    assert_eq!(engine.visual.offset(timeline), engine_range);
    wheel(&mut session, &device, engine_point, -40);
    let enginemark = evaluate(&mut session, "scenario.position();");
    assert_ne!(enginemark, "end");

    select(&mut session, "general-channel");
    assert_eq!(evaluate(&mut session, "scenario.position();"), generalmark);
    select(&mut session, "engine-channel");
    assert_eq!(evaluate(&mut session, "scenario.position();"), enginemark);
    let generalset = evaluate(&mut session, "scenario.saved('general');");
    let key = generalmark.split_once(':').unwrap().0;
    let ordinal = key
        .strip_prefix("chat-message-")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let fallback = if ordinal < 32 {
        ordinal + 1
    } else {
        ordinal - 1
    };
    let prefix = format!("chat-message-{fallback}:");
    let expected = generalset
        .split(" | ")
        .find(|position| position.starts_with(&prefix))
        .unwrap()
        .to_owned();
    assert_eq!(
        evaluate(&mut session, &format!("scenario.drop('general', '{key}');")),
        "true"
    );
    select(&mut session, "general-channel");
    assert!(
        evaluate(&mut session, "scenario.positions();")
            .split(" | ")
            .any(|position| position == expected)
    );

    insert(&mut session, "general draft");
    select(&mut session, "engine-channel");
    assert_eq!(messages(&snapshot(&mut session)), 32);
    assert_eq!(value(&mut session), "");

    insert(&mut session, "engine draft");
    select(&mut session, "general-channel");
    assert_eq!(messages(&snapshot(&mut session)), 31);
    assert_eq!(value(&mut session), "general draft");
    commit(&mut session, "");
    assert_eq!(messages(&snapshot(&mut session)), 32);
    assert_eq!(value(&mut session), "");

    select(&mut session, "engine-channel");
    assert_eq!(messages(&snapshot(&mut session)), 32);
    assert_eq!(value(&mut session), "engine draft");
    commit(&mut session, "");
    assert_eq!(messages(&snapshot(&mut session)), 33);

    select(&mut session, "general-channel");
    assert_eq!(messages(&snapshot(&mut session)), 32);
    assert_eq!(value(&mut session), "");
    call(&mut session, Command::Shutdown {});
    assert!(device.0.borrow().windows.is_empty());
}
