use crate::device::{self, Device};
use crate::web::snapshot;
use plectron::addons::control::Pump;
use plectron::ipc::Session;
use plectron::platform::Host;

#[test]
fn entry() {
    let (mut runtime, _device): (_, Device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/lifecycle/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let page = snapshot(&mut session);
    assert!(page.resources.iter().any(|resource| {
        resource.url.ends_with("/lifecycle/main.js") && resource.mime == "text/javascript"
    }));
    assert!(
        page.nodes
            .iter()
            .any(|node| node.content == "A component owns its DOM.")
    );
    assert!(
        page.nodes
            .iter()
            .any(|node| node.content == "Its child has an independent lifecycle.")
    );
}
