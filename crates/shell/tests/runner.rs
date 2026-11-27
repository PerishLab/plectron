use platform::shared::{
    Clipboard, Console, Dialog as Native, Display, Event, Image, Input, Mode, Outcome, Signal,
    Unit, Window,
};
use shell::{Bundle, Dialog, Options, Prompt, Runner, Selection, Status, Value};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use webengine::Fault;

struct Store(Mutex<Option<String>>);

impl Clipboard for Store {
    fn read(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn write(&self, text: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(text.into());
        Ok(())
    }
}

struct Stub {
    frames: usize,
    closed: bool,
    signals: VecDeque<Signal>,
    dialogs: Vec<Native>,
    titles: Vec<String>,
    clip: Option<Arc<Store>>,
    autoclose: bool,
}

impl Display<Fault> for Stub {
    fn create(&mut self, _: Unit, _: &Window) -> Result<(), Fault> {
        Ok(())
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), Fault> {
        self.titles.push(title.into());
        if title == "pasted" {
            self.signals.push_back(key(unit, "Primary+KeyC"));
        }
        Ok(())
    }
    fn dialog(&mut self, _: Unit, request: &Native) -> Result<Outcome, Fault> {
        self.dialogs.push(request.clone());
        match request.mode {
            Mode::Open => Ok(Outcome::Selected(vec![PathBuf::from("C:\\native\\a.pbnd")])),
            Mode::Save => Ok(Outcome::Selected(vec![PathBuf::from("C:\\native\\b.log")])),
            Mode::Folder => Ok(Outcome::Canceled),
        }
    }
    fn present(&mut self, unit: Unit, _: Image) -> Result<(), Fault> {
        self.frames += 1;
        if self.autoclose {
            self.signals.push_back(Signal {
                unit,
                event: Event::Close,
            });
        } else if self.frames == 1 && self.clip.is_some() {
            self.signals.push_back(key(unit, "Primary+KeyV"));
        }
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
        if let Some(store) = &self.clip
            && self.dialogs.len() == 3
            && store.0.lock().unwrap().as_deref() == Some("ast")
        {
            self.signals.push_back(Signal {
                unit: Unit::new(1),
                event: Event::Close,
            });
        }
        Ok(self.signals.drain(..).collect())
    }

    fn clipboard(&self) -> Option<Arc<dyn Clipboard>> {
        self.clip
            .as_ref()
            .map(|store| store.clone() as Arc<dyn Clipboard>)
    }
}

impl Console<Fault> for Stub {
    fn log(&mut self, _: &str) -> Result<(), Fault> {
        Ok(())
    }
}

struct Custom;

impl Dialog for Custom {
    fn open(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Paths(vec!["/override/custom.txt".into()]))
    }
    fn save(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Cancelled)
    }
    fn folder(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Cancelled)
    }
}

fn key(unit: Unit, chord: &str) -> Signal {
    Signal {
        unit,
        event: Event::Input(shared::shortcut::Input::Key(shared::shortcut::Stroke {
            chord: shared::shortcut::Chord::parse(chord).unwrap(),
            repeat: false,
            text: String::new(),
        })),
    }
}

fn page(script: &str) -> Bundle {
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
        .insert("/app.js", "text/javascript", script.as_bytes().to_vec())
        .unwrap();
    bundle
}

#[test]
fn lifecycle() {
    let bundle = page("Symbol[Symbol.plectron].document.getElementById('app').setText('ready');");
    let stub = Stub {
        frames: 0,
        closed: false,
        signals: VecDeque::new(),
        dialogs: Vec::new(),
        titles: Vec::new(),
        clip: None,
        autoclose: true,
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

#[test]
fn native() {
    let script = r#"
const doc = Symbol[Symbol.plectron].document;
const bridge = Symbol[Symbol.plectron].bridge;
const field = doc.createElement('input');
field.setAttribute('id', 'field');
doc.getElementById('app').appendChild(field);
field.focus();
field.addEventListener('input', () => {
  if (field.getValue() === 'pasted') {
    field.setSelection(1, 4);
    bridge.postMessage('{"kind":"command","command_id":"d-paste","command":"window.title","payload":{"title":"pasted"}}');
  }
});
bridge.postMessage('{"kind":"command","command_id":"d-1","command":"dialog.openFile","payload":{"title":"Open"}}');
bridge.postMessage('{"kind":"command","command_id":"d-2","command":"dialog.saveFile","payload":{"name":"b.log"}}');
bridge.postMessage('{"kind":"command","command_id":"d-3","command":"dialog.pickFolder","payload":null}');
"#;
    let store = Arc::new(Store(Mutex::new(Some("pasted".into()))));
    let stub = Stub {
        frames: 0,
        closed: false,
        signals: VecDeque::new(),
        dialogs: Vec::new(),
        titles: Vec::new(),
        clip: Some(store.clone()),
        autoclose: false,
    };
    let mut runner = Runner::open(stub, page(script), Options::new("Native", 320, 200)).unwrap();
    for _ in 0..300 {
        if !runner.active() {
            break;
        }
        runner.tick().unwrap();
        if runner.active() {
            runner.wait().unwrap();
        }
    }
    assert!(!runner.active());
    assert_eq!(runner.backend().dialogs.len(), 3);
    assert_eq!(runner.backend().dialogs[0].mode, Mode::Open);
    assert_eq!(runner.backend().dialogs[1].mode, Mode::Save);
    assert_eq!(runner.backend().dialogs[2].mode, Mode::Folder);
    assert_eq!(runner.backend().titles, vec!["pasted"]);
    assert_eq!(store.0.lock().unwrap().as_deref(), Some("ast"));
    assert_eq!(
        runner.ledger().query("d-1").map(|r| r.status),
        Some(Status::Completed)
    );
    assert_eq!(
        runner
            .ledger()
            .query("d-1")
            .and_then(|r| r.value.as_ref())
            .and_then(|v| v.field("paths"))
            .and_then(Value::array)
            .and_then(|a| a.first())
            .and_then(Value::text),
        Some("C:\\native\\a.pbnd")
    );
}

#[test]
fn custom() {
    let script = r#"
const bridge = Symbol[Symbol.plectron].bridge;
bridge.onMessage(() => {
  bridge.postMessage('{"kind":"command","command_id":"c-2","command":"window.close"}');
});
bridge.postMessage('{"kind":"command","command_id":"c-1","command":"dialog.openFile","payload":{}}');
"#;
    let stub = Stub {
        frames: 0,
        closed: false,
        signals: VecDeque::new(),
        dialogs: Vec::new(),
        titles: Vec::new(),
        clip: None,
        autoclose: false,
    };
    let options = Options::new("Custom", 320, 200).dialog(Custom);
    let mut runner = Runner::open(stub, page(script), options).unwrap();
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
    assert!(runner.backend().dialogs.is_empty());
    assert_eq!(
        runner
            .ledger()
            .query("c-1")
            .and_then(|r| r.value.as_ref())
            .and_then(|v| v.field("paths"))
            .and_then(Value::array)
            .and_then(|a| a.first())
            .and_then(Value::text),
        Some("/override/custom.txt")
    );
}
