use platform::shared::{Console, Display, Image, Input, Progress, Region, Signal, Unit, Window};
use shell::{
    Addon, Bundle, Context, Dialog, Options, Outcome, Prompt, Request, Runner, Selection, Status,
    Update, Value,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use webengine::Fault;

struct Stub {
    titles: Vec<String>,
    regions: Vec<Vec<Region>>,
    progresses: Vec<Progress>,
    logs: Vec<String>,
    closed: bool,
}

impl Display<Fault> for Stub {
    fn create(&mut self, _: Unit, _: &Window) -> Result<(), Fault> {
        Ok(())
    }

    fn title(&mut self, _: Unit, title: &str) -> Result<(), Fault> {
        self.titles.push(title.into());
        Ok(())
    }

    fn drag(&mut self, _: Unit, regions: &[Region]) -> Result<(), Fault> {
        self.regions.push(regions.to_vec());
        Ok(())
    }

    fn progress(&mut self, _: Unit, progress: Progress) -> Result<(), Fault> {
        self.progresses.push(progress);
        Ok(())
    }

    fn present(&mut self, _: Unit, _: Image) -> Result<(), Fault> {
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
        Ok(Vec::new())
    }
}

impl Console<Fault> for Stub {
    fn log(&mut self, text: &str) -> Result<(), Fault> {
        self.logs.push(text.into());
        Ok(())
    }
}

struct Picker;

impl Dialog for Picker {
    fn open(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Paths(vec!["/install/package.pbnd".into()]))
    }

    fn save(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Paths(vec!["/install/log.txt".into()]))
    }

    fn folder(&mut self, _: &Prompt) -> Result<Selection, String> {
        Ok(Selection::Paths(vec!["/opt/plectron".into()]))
    }
}

struct Installer {
    count: Arc<AtomicUsize>,
}

impl Addon for Installer {
    fn namespace(&self) -> &str {
        "installer"
    }

    fn execute(&mut self, request: &Request<'_>, _: &mut Context<'_>) -> Outcome {
        self.count.fetch_add(1, Ordering::SeqCst);
        match request.command {
            "installer.plan" => Outcome::Completed(Value::Text("ready".into())),
            "installer.commit" => Outcome::Unknown("commit ack lost".into()),
            "installer.copy" => Outcome::Running(Some(Update {
                current: 1,
                total: Some(2),
                message: Some("copying".into()),
            })),
            _ => Outcome::Rejected("unknown installer command".into()),
        }
    }

    fn cancel(&mut self, id: &str, _: &mut Context<'_>) -> Option<Outcome> {
        Some(Outcome::Rejected(format!("cancelled {id}")))
    }
}

#[test]
fn roundtrip() {
    let script = r#"
const bridge = Symbol[Symbol.plectron].bridge;
let stage = 0;
let seenUnknownDup = false;
bridge.onMessage((raw) => {
  if (stage === 0 && raw === '{"command":"window.title","command_id":"c-title","duplicate":false,"error":null,"found":true,"kind":"receipt","progress":null,"status":"completed","value":null}') {
    stage = 1;
    bridge.postMessage('{"kind":"command","command_id":"c-drag","command":"window.drag","payload":{"regions":[{"left":0,"top":0,"right":240,"bottom":32}]}}');
    bridge.postMessage('{"kind":"command","command_id":"c-prog","command":"window.progress","payload":{"state":"normal","current":25,"total":100}}');
    bridge.postMessage('{"kind":"command","command_id":"c-dir","command":"dialog.pickFolder","payload":{"title":"Target"}}');
    bridge.postMessage('{"kind":"command","command_id":"c-plan","command":"installer.plan","payload":{}}');
    bridge.postMessage('{"kind":"command","command_id":"c-plan","command":"installer.plan","payload":{}}');
    bridge.postMessage('{"kind":"command","command_id":"c-commit","command":"installer.commit","payload":{}}');
    bridge.postMessage('{"kind":"command","command_id":"c-commit","command":"installer.commit","payload":{}}');
    bridge.postMessage('{"kind":"command","command_id":"c-copy","command":"installer.copy","payload":{}}');
    return;
  }
  if (raw === '{"command":"installer.commit","command_id":"c-commit","duplicate":true,"error":"commit ack lost","found":true,"kind":"receipt","progress":null,"status":"unknown","value":null}') {
    seenUnknownDup = true;
    return;
  }
  if (stage === 1 && seenUnknownDup && raw === '{"command":"installer.copy","command_id":"c-copy","duplicate":false,"error":null,"found":true,"kind":"receipt","progress":{"current":1,"message":"copying","total":2},"status":"running","value":null}') {
    stage = 2;
    bridge.postMessage('{"kind":"query","command_id":"c-commit"}');
    bridge.postMessage('{"kind":"cancel","command_id":"c-copy"}');
    return;
  }
  if (stage === 2 && raw === '{"command":"installer.copy","command_id":"c-copy","duplicate":false,"error":"cancelled c-copy","found":true,"kind":"receipt","progress":{"current":1,"message":"copying","total":2},"status":"rejected","value":null}') {
    stage = 3;
    bridge.postMessage('{"kind":"command","command_id":"c-close","command":"window.close"}');
  }
});
bridge.postMessage('{"kind":"command","command_id":"c-title","command":"window.title","payload":{"title":"Plectron Setup"}}');
"#;

    let mut bundle = Bundle::new("app://shell/", "app://shell/index.html").unwrap();
    bundle
        .insert(
            "/index.html",
            "text/html",
            b"<!doctype html><html><head><link rel='stylesheet' href='app.css'><script src='app.js'></script></head><body><div id='app'>setup</div></body></html>".to_vec(),
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

    let count = Arc::new(AtomicUsize::new(0));
    let options = Options::new("Setup", 320, 200)
        .dialog(Picker)
        .addon(Installer {
            count: count.clone(),
        });
    let stub = Stub {
        titles: Vec::new(),
        regions: Vec::new(),
        progresses: Vec::new(),
        logs: Vec::new(),
        closed: false,
    };
    let mut runner = Runner::open(stub, bundle, options).unwrap();
    for _ in 0..300 {
        if !runner.active() {
            break;
        }
        runner.tick().unwrap();
        if runner.active() {
            runner.wait().unwrap();
        }
    }

    assert!(
        runner.backend().logs.is_empty(),
        "{:?}",
        runner.backend().logs
    );
    assert!(!runner.active());
    assert!(runner.backend().closed);
    assert_eq!(runner.backend().titles, vec!["Plectron Setup"]);
    assert_eq!(
        runner.backend().regions,
        vec![vec![Region {
            left: 0,
            top: 0,
            right: 240,
            bottom: 32,
        }]]
    );
    assert_eq!(
        runner.backend().progresses,
        vec![Progress::Normal {
            current: 25,
            total: 100,
        }]
    );
    assert_eq!(count.load(Ordering::SeqCst), 3);
    assert_eq!(
        runner.ledger().query("c-dir").map(|r| r.status),
        Some(Status::Completed)
    );
    assert_eq!(
        runner.ledger().query("c-plan").map(|r| r.status),
        Some(Status::Completed)
    );
    assert_eq!(
        runner.ledger().query("c-commit").map(|r| r.status),
        Some(Status::Unknown)
    );
    assert_eq!(
        runner.ledger().query("c-copy").map(|r| r.status),
        Some(Status::Rejected)
    );
    assert_eq!(
        runner.ledger().query("c-close").map(|r| r.status),
        Some(Status::Completed)
    );
}
