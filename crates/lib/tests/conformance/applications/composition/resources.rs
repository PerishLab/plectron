use crate::device::{self, Device};
use crate::web::{call, snapshot};
use plectron::addons::control::Pump;
use plectron::ipc::{Command, Reply, Session};
use plectron::platform::Host;
use plectron::rendering::Solid;
use webengine::State;

struct Application {
    session: Session<Device, Solid>,
}

impl Application {
    fn new(app: &str) -> Self {
        let (mut runtime, _) = device::runtime();
        let root = fixtures::root();
        let source = crate::application(&format!("apps/{app}/start.ts"))
            .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
        runtime.execute(&source).unwrap();
        runtime.addons().host.finish().unwrap();
        runtime.poll().unwrap();
        let mut session = Session::new(runtime, 42, false);
        let page = snapshot(&mut session);
        assert_eq!(page.state, State::Ready, "{:?}", page.fault);
        assert!(page.resources.iter().any(|resource| {
            resource.url.ends_with(&format!("/{app}/main.ts")) && resource.mime == "text/javascript"
        }));
        Self { session }
    }

    fn evaluate(&mut self, source: &str) -> String {
        let Reply::Evaluation { display } = call(
            &mut self.session,
            Command::Evaluate {
                unit: 1,
                source: source.into(),
            },
        ) else {
            panic!()
        };
        display
    }

    fn text(&mut self, id: &str) -> String {
        let page = snapshot(&mut self.session);
        let parent = page
            .nodes
            .iter()
            .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
            .unwrap()
            .id;
        page.nodes
            .iter()
            .filter(|node| node.parent == Some(parent))
            .map(|node| node.content.as_str())
            .collect()
    }

    fn wait(&mut self, expected: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self.text("status") != expected {
            assert!(
                std::time::Instant::now() < deadline,
                "expected {expected}; observed {}",
                self.text("status")
            );
            self.session.tick().unwrap();
            std::thread::yield_now();
        }
    }

    fn shutdown(&mut self) {
        assert!(snapshot(&mut self.session).jobs.rejected.is_empty());
        let Reply::Shutdown { errors } = call(&mut self.session, Command::Shutdown {}) else {
            panic!()
        };
        assert!(errors.is_empty(), "{errors:?}");
        assert!(!self.session.active());
    }
}

#[test]
fn request() {
    let mut app = Application::new("request");
    app.evaluate("document.getElementById('load').dispatchEvent('click', '');");
    app.evaluate("ping.dispatchEvent('click', '');");
    assert_eq!(app.evaluate("clicks;"), "1");
    app.wait("HTTP 200");
    let expected = std::fs::read_to_string(fixtures::root().join("request/message.txt")).unwrap();
    assert_eq!(app.text("result"), expected);
    app.evaluate(
        "url.setValue('./missing.txt'); document.getElementById('load').dispatchEvent('click', '');",
    );
    app.wait("HTTP 404");
    assert!(snapshot(&mut app.session).jobs.rejected.is_empty());
    app.shutdown();
}

#[test]
fn network() {
    let mut app = Application::new("network");
    assert_eq!(
        app.evaluate("picture.getAttribute('src');"),
        "../pictures/dawn.png"
    );
    app.evaluate("document.getElementById('swap').dispatchEvent('click', '');");
    app.session.tick().unwrap();
    assert_eq!(
        app.evaluate("picture.getAttribute('src');"),
        "../pictures/dusk.png"
    );
    assert!(
        snapshot(&mut app.session)
            .nodes
            .iter()
            .any(|node| node.content == "Dusk loaded. Font resources retained.")
    );
    app.evaluate("document.getElementById('swap').dispatchEvent('click', '');");
    app.session.tick().unwrap();
    assert_eq!(
        app.evaluate("picture.getAttribute('src');"),
        "../pictures/dawn.png"
    );
    assert!(snapshot(&mut app.session).jobs.rejected.is_empty());
    app.shutdown();
}
