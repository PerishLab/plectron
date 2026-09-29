use super::device;
use plectron::addons::control::{Dispatch, Pump};
use plectron::contract::{Event, Signal};
use plectron::platform::Host;

#[test]
fn counter() {
    let (mut runtime, device) = device::runtime();
    runtime
        .execute(&crate::application("apps/counter/start.js"))
        .unwrap();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    let signal = Signal {
        unit,
        event: Event::Click,
    };
    runtime.dispatch(signal.clone()).unwrap();
    runtime.dispatch(signal.clone()).unwrap();
    let record = device.0.borrow();
    assert_eq!(
        record.titles,
        ["Plectron — clicks: 1", "Plectron — clicks: 2"]
    );
    assert_eq!(record.logs, ["click 1", "click 2"]);
    assert_eq!(record.frames.len(), 3);
    assert_ne!(record.frames[0].pixels[0], record.frames[1].pixels[0]);
    assert_eq!(record.frames[0], record.frames[2]);
}

#[test]
fn retained() {
    let (mut runtime, device) = device::runtime();
    runtime
        .execute(&crate::application("apps/closure/start.js"))
        .unwrap();
    let units: Vec<_> = device.0.borrow().windows.keys().copied().collect();
    for _ in 0..3 {
        runtime
            .dispatch(Signal {
                unit: units[0],
                event: Event::Click,
            })
            .unwrap();
    }
    assert!(runtime.addons().host.check(units[0]).is_err());
    assert!(runtime.addons().host.check(units[1]).is_ok());
    for signal in runtime.addons().host.poll().unwrap() {
        runtime.dispatch(signal.clone()).unwrap();
    }
    runtime
        .dispatch(Signal {
            unit: units[1],
            event: Event::Click,
        })
        .unwrap();
    let record = device.0.borrow();
    assert_eq!(record.logs, ["First — 3 clicks to close closed at 3"]);
    assert_eq!(
        record.titles.last().unwrap(),
        "Second — 3 clicks to close: 1"
    );
}

#[test]
fn invalidation() {
    let (mut runtime, device) = device::control();
    runtime
        .execute("const w = platform.window({}); w.close();")
        .unwrap();
    let failure = runtime.execute("w.title('stale');").unwrap_err();
    let error = runtime.diagnostic(&failure);
    assert!(error.0.contains("closed"));
    assert!(device.0.borrow().titles.is_empty());
    assert!(runtime.execute("w.on('click', function () {});").is_err());
}

#[test]
fn physical() {
    let (mut runtime, device) = device::control();
    runtime
        .execute(
            "const w = platform.window({}); w.on('close', function () { console.log('closed'); });",
        )
        .unwrap();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    let signal = Signal {
        unit,
        event: Event::Close,
    };
    device
        .0
        .borrow_mut()
        .signals
        .extend([device::close(unit), device::close(unit)]);
    let events = runtime.addons().host.poll().unwrap();
    assert_eq!(events, std::slice::from_ref(&signal));
    assert!(runtime.addons().host.check(unit).is_err());
    for event in events {
        runtime.dispatch(event).unwrap();
    }
    runtime.dispatch(signal.clone()).unwrap();
    assert_eq!(device.0.borrow().logs, ["closed"]);
    assert_eq!(device.0.borrow().closed, [unit]);
    assert!(!runtime.active());
}

#[test]
fn callback() {
    let (mut runtime, device) = device::control();
    runtime
        .execute("const w = platform.window({}); w.on('click', function () { missing(); });")
        .unwrap();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    let error = runtime
        .dispatch(Signal {
            unit,
            event: Event::Click,
        })
        .unwrap_err();
    assert!(error.0.contains("unknown binding"));
}

#[test]
fn native() {
    for source in [
        "platform.window({width: 0});",
        "platform.window({width: 2.5});",
        "platform.window({widht: 100});",
        "platform.window(1);",
        "platform.window({width: 4096, height: 4096});",
    ] {
        let (mut runtime, device) = device::control();
        assert!(runtime.execute(source).is_err());
        assert!(device.0.borrow().windows.is_empty());
    }
}

#[test]
fn failure() {
    let (mut runtime, device) = device::control();
    device.0.borrow_mut().fail = true;
    let failure = runtime.execute("platform.window({});").unwrap_err();
    let error = runtime.diagnostic(&failure);
    assert_eq!(error.0, "creation rejected");
    assert!(!runtime.active());
}

#[test]
fn authority() {
    let (mut runtime, _) = device::control();
    runtime.execute("const w = platform.window({});").unwrap();
    let failure = runtime.execute("w.filesystem();").unwrap_err();
    let error = runtime.diagnostic(&failure);
    assert!(error.0.contains("not callable"));
}

#[test]
fn ordering() {
    let (mut runtime, device) = device::control();
    runtime.execute("const w = platform.window({}); w.on('click', function () { console.log('click'); }); w.on('close', function () { console.log('close'); });").unwrap();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.extend(device::click(unit));
    device.0.borrow_mut().signals.push(device::close(unit));
    while runtime.active() {
        for signal in runtime.addons().host.poll().unwrap() {
            runtime.dispatch(signal.clone()).unwrap();
        }
    }
    assert_eq!(device.0.borrow().logs, ["click", "close"]);
}

#[test]
fn stale() {
    let (mut runtime, device) = device::control();
    runtime.execute("const w = platform.window({}); w.on('click', function () { console.log('click'); w.close(); }); w.on('close', function () { console.log('close'); });").unwrap();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    let signal = Signal {
        unit,
        event: Event::Click,
    };
    device
        .0
        .borrow_mut()
        .signals
        .extend(device::click(unit).into_iter().chain(device::click(unit)));
    while runtime.active() {
        for event in runtime.addons().host.poll().unwrap() {
            runtime.dispatch(event).unwrap();
        }
    }
    runtime.dispatch(signal.clone()).unwrap();
    assert_eq!(device.0.borrow().logs, ["click", "close"]);
}
