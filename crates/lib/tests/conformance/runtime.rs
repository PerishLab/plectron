use super::device;
use jsengine::Value;
use plectron::addons::surface::Surface;
use plectron::contract::Extent;
use plectron::runtime::Runtime;

fn surface(extent: Extent) -> Runtime<Surface> {
    let mut runtime = Runtime::new(Surface::new(extent).unwrap()).unwrap();
    runtime
        .execute(&fixtures::script("surface, platform, console", ""))
        .unwrap();
    runtime
}

#[test]
fn profiles() {
    let (mut control, device) = device::control();
    let mut surface = surface(Extent {
        width: 3,
        height: 2,
    });
    control.execute("const w = platform.window({});").unwrap();
    surface.execute("surface.fill(255);").unwrap();
    assert_eq!(
        surface.execute("platform === undefined;").unwrap(),
        Value::Boolean(true)
    );
    assert!(surface.execute("console.log('no authority');").is_err());
    assert_eq!(
        control.execute("surface === undefined;").unwrap(),
        Value::Boolean(true)
    );
    assert_eq!(device.0.borrow().windows.len(), 1);
    assert!(device.0.borrow().frames.is_empty());
    let frame = surface.addons().take().unwrap();
    assert_eq!(
        frame.extent,
        Extent {
            width: 3,
            height: 2
        }
    );
    assert_eq!(frame.pixels, [255; 6]);
    assert!(surface.addons().take().is_none());
}

#[test]
fn isolation() {
    let (mut control, _) = device::control();
    let mut surface = surface(Extent {
        width: 1,
        height: 1,
    });
    control.execute("let state = 10;").unwrap();
    surface.execute("let state = 20;").unwrap();
    let callback = control
        .execute("(function () { state = state + 1; return state; });")
        .unwrap();
    assert!(
        surface
            .call(callback.clone(), Vec::new())
            .unwrap_err()
            .to_string()
            .contains("another engine")
    );
    assert_eq!(
        control.call(callback, Vec::new()).unwrap(),
        Value::Number(11.0)
    );
    assert_eq!(surface.execute("state;").unwrap(), Value::Number(20.0));
}

#[test]
fn extents() {
    let mut small = surface(Extent {
        width: 2,
        height: 1,
    });
    let mut large = surface(Extent {
        width: 4,
        height: 3,
    });
    let script = "surface.fill(surface.width * surface.height);";
    small.execute(script).unwrap();
    large.execute(script).unwrap();
    assert_eq!(small.addons().take().unwrap().pixels, [2; 2]);
    assert_eq!(large.addons().take().unwrap().pixels, [12; 12]);
}

#[test]
fn refusals() {
    assert!(
        Surface::new(Extent {
            width: 0,
            height: 1
        })
        .is_err()
    );
    let mut surface = surface(Extent {
        width: 2,
        height: 2,
    });
    assert!(surface.execute("surface.fill(-1);").is_err());
    assert!(surface.execute("surface.fill(1, 2);").is_err());
    assert!(surface.execute("surface.width = 999;").is_err());
    assert!(surface.addons().take().is_none());
}

#[test]
fn exceptions() {
    let (mut control, device) = device::control();
    control.execute("try { missing; } catch (error) { console.log(error.name); } finally { console.log('finished'); }").unwrap();
    assert_eq!(device.0.borrow().logs, ["ReferenceError", "finished"]);
    let mut surface = surface(Extent {
        width: 2,
        height: 2,
    });
    surface.execute("try { surface.width = 7; } catch (error) { if (error.constructor === TypeError) { surface.fill(7); } }").unwrap();
    assert_eq!(surface.addons().take().unwrap().pixels, [7; 4]);
    let failure = surface
        .execute(
            "try { surface.fill(-1); } catch { surface.fill(1); } finally { surface.fill(2); } ",
        )
        .unwrap_err();
    assert!(matches!(failure, jsengine::Failure::Fault(_)));
    assert!(surface.addons().take().is_none());
}

#[test]
fn receivers() {
    let (mut control, device) = device::control();
    let receiver = control
        .execute("({message: 'receiver', send: console.log});")
        .unwrap();
    let callback = control
        .execute("(function () { this.send(this.message); });")
        .unwrap();
    control
        .invoke(
            callback,
            jsengine::Invocation {
                receiver,
                arguments: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(device.0.borrow().logs, ["receiver"]);
    control.execute("const first = platform.window({title: 'first'}); const second = platform.window({title: 'second'});").unwrap();
    let first = *device.0.borrow().windows.keys().next().unwrap();
    control
        .execute("const alias = {close: first.close}; alias.close();")
        .unwrap();
    let record = device.0.borrow();
    assert_eq!(record.closed, [first]);
    assert_eq!(record.windows.len(), 1);
}

#[test]
fn construction() {
    let (mut control, device) = device::control();
    let constructor = control.execute("const C = function (message) { this.message = message; }; C.prototype.send = function () { console.log(this.message); }; C;").unwrap();
    let instance = control
        .construct(constructor.clone(), vec![Value::Text("constructed".into())])
        .unwrap();
    control
        .host(|engine, _| engine.register("instance", instance.clone()))
        .unwrap();
    control.execute("instance.send();").unwrap();
    assert_eq!(device.0.borrow().logs, ["constructed"]);
    let (mut other, _) = device::control();
    assert!(matches!(
        other.construct(constructor, Vec::new()),
        Err(jsengine::Failure::Fault(_))
    ));
    let local = other
        .execute("(function (value) { this.value = value; });")
        .unwrap();
    assert!(matches!(
        other.construct(local, vec![instance]),
        Err(jsengine::Failure::Fault(_))
    ));
}

#[test]
fn classes() {
    use plectron::addons::control::Dispatch;
    use plectron::contract::{Event, Signal};
    let (mut runtime, device) = device::control();
    runtime.execute("class Count { storage = 0; get count() { return this.storage; } set count(n) { this.storage = n; } increment(step = 1) { this.count += step; } } class Counter extends Count { constructor(label = 'first') { super(); this.label = label; this.window = platform.window({title: label}); this.window.on('click', () => this.increment()); } increment() { super.increment(); this.window.title(this.label + ':' + this.count); } } const a = new Counter(); const b = new Counter('second');").unwrap();
    let units: Vec<_> = device.0.borrow().windows.keys().copied().collect();
    for unit in [units[0], units[0], units[1]] {
        runtime
            .dispatch(Signal {
                unit,
                event: Event::Click,
            })
            .unwrap();
    }
    assert_eq!(device.0.borrow().titles, ["first:1", "first:2", "second:1"]);
    assert_eq!(
        runtime.execute("a.count === 2 && b.count === 1;").unwrap(),
        Value::Boolean(true)
    );
    assert!(runtime.execute("try { class Refusal { window = platform.window({width: 0}); } new Refusal(); } catch { console.log('caught'); }").is_err());
    assert!(device.0.borrow().logs.is_empty());
}

#[test]
fn records() {
    let (mut runtime, device) = device::control();
    assert!(runtime.execute("let effects = 0; try { platform.window({get title() { effects = effects + 1; return 'getter'; }}); } catch { effects = effects + 10; }").is_err());
    assert_eq!(runtime.execute("effects;").unwrap(), Value::Number(0.0));
    assert!(device.0.borrow().windows.is_empty());
    runtime.execute("const options = {get title() { return 'explicit'; }}; platform.window({title: options.title});").unwrap();
    assert_eq!(device.0.borrow().windows.len(), 1);
}
