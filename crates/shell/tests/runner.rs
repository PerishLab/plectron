use platform::shared::{Console, Display, Event, Image, Input, Signal, Unit, Window};
use shell::{Bundle, Options, Runner};
use std::collections::VecDeque;
use webengine::Fault;

struct Stub {
    frames: usize,
    closed: bool,
    signals: VecDeque<Signal>,
}

impl Display<Fault> for Stub {
    fn create(&mut self, _: Unit, _: &Window) -> Result<(), Fault> {
        Ok(())
    }
    fn title(&mut self, _: Unit, _: &str) -> Result<(), Fault> {
        Ok(())
    }
    fn present(&mut self, unit: Unit, _: Image) -> Result<(), Fault> {
        self.frames += 1;
        self.signals.push_back(Signal {
            unit,
            event: Event::Close,
        });
        Ok(())
    }
    fn wait(&mut self, _: graphics::shared::Pacing) -> Result<(), Fault> {
        std::thread::sleep(std::time::Duration::from_millis(5));
        Ok(())
    }
    fn close(&mut self, _: Unit) -> Result<(), Fault> {
        self.closed = true;
        Ok(())
    }
}

impl Input<Fault> for Stub {
    fn poll(&mut self) -> Result<Vec<Signal>, Fault> {
        Ok(self.signals.drain(..).collect())
    }
}

impl Console<Fault> for Stub {
    fn log(&mut self, _: &str) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn lifecycle() {
    let mut bundle = Bundle::new("app://shell/", "app://shell/index.html").unwrap();
    bundle
        .insert(
            "/index.html",
            "text/html",
            b"<!doctype html><html><head><link rel='stylesheet' href='app.css'><script src='app.js'></script></head><body><div id='app'>hello</div></body></html>".to_vec(),
        )
        .unwrap();
    bundle
        .insert(
            "/app.css",
            "text/css",
            b"body { margin: 0; background-color: #ffffff; color: #111111; }".to_vec(),
        )
        .unwrap();
    bundle
        .insert(
            "/app.js",
            "text/javascript",
            b"Symbol[Symbol.plectron].document.getElementById('app').setText('ready');".to_vec(),
        )
        .unwrap();
    let stub = Stub {
        frames: 0,
        closed: false,
        signals: VecDeque::new(),
    };
    let mut runner = Runner::open(stub, bundle, Options::new("Probe", 320, 200)).unwrap();
    for _ in 0..200 {
        if !runner.active() {
            break;
        }
        runner.tick().unwrap();
        if runner.active() {
            runner.wait().unwrap();
        }
    }
    assert!(!runner.active());
}
