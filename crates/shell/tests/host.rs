use platform::shared::{Progress, Region};
use shell::control::json::{LIMIT, encode, parse};
use shell::control::wire::{Reply, encode as frame};
use shell::{Broker, Dialog, Host, Prompt, Receipt, Selection, Status, Surface, Update, Value};
use std::collections::BTreeMap;

#[derive(Default)]
struct Stub {
    titles: Vec<String>,
    regions: Vec<Vec<Region>>,
    progresses: Vec<Progress>,
    closed: bool,
}

impl Surface for Stub {
    fn title(&mut self, title: &str) -> Result<(), String> {
        self.titles.push(title.into());
        Ok(())
    }

    fn drag(&mut self, regions: &[Region]) -> Result<(), String> {
        self.regions.push(regions.to_vec());
        Ok(())
    }

    fn progress(&mut self, progress: Progress) -> Result<(), String> {
        self.progresses.push(progress);
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.closed = true;
        Ok(())
    }
}

struct Picker {
    opened: Vec<Prompt>,
    saved: Vec<Prompt>,
    picked: Vec<Prompt>,
}

impl Dialog for Picker {
    fn open(&mut self, prompt: &Prompt) -> Result<Selection, String> {
        self.opened.push(prompt.clone());
        Ok(Selection::Paths(vec!["/tmp/a.txt".into()]))
    }

    fn save(&mut self, prompt: &Prompt) -> Result<Selection, String> {
        self.saved.push(prompt.clone());
        Ok(Selection::Paths(vec!["/tmp/out.bin".into()]))
    }

    fn folder(&mut self, prompt: &Prompt) -> Result<Selection, String> {
        self.picked.push(prompt.clone());
        Ok(Selection::Cancelled)
    }
}

#[test]
fn host() {
    let mut broker = Broker::default();
    let picker = Picker {
        opened: Vec::new(),
        saved: Vec::new(),
        picked: Vec::new(),
    };
    broker
        .register(Box::new(Host::new(Some(Box::new(picker)))))
        .unwrap();
    let mut surface = Stub::default();

    assert_eq!(
        broker
            .receive(
                r#"{"kind":"command","command_id":"h-1","command":"window.title","payload":{"title":"Installer"}}"#,
                &mut surface,
            )
            .status,
        Status::Completed
    );
    assert_eq!(surface.titles, vec!["Installer"]);

    assert_eq!(
        broker
            .receive(
                r#"{"kind":"command","command_id":"h-2","command":"window.drag","payload":{"regions":[{"left":0,"top":0,"right":320,"bottom":36}]}}"#,
                &mut surface,
            )
            .status,
        Status::Completed
    );
    assert_eq!(
        surface.regions,
        vec![vec![Region {
            left: 0,
            top: 0,
            right: 320,
            bottom: 36,
        }]]
    );

    assert_eq!(
        broker
            .receive(
                r#"{"kind":"command","command_id":"h-3","command":"window.progress","payload":{"state":"normal","current":50,"total":100}}"#,
                &mut surface,
            )
            .status,
        Status::Completed
    );
    assert_eq!(
        surface.progresses,
        vec![Progress::Normal {
            current: 50,
            total: 100,
        }]
    );

    let open = broker.receive(
        r#"{"kind":"command","command_id":"h-4","command":"dialog.openFile","payload":{"title":"Pick","multiple":true,"filters":[{"label":"Text","extensions":["txt"]}]}}"#,
        &mut surface,
    );
    assert_eq!(open.status, Status::Completed);
    assert_eq!(
        open.value,
        Some(Value::Object(BTreeMap::from([
            ("cancelled".into(), Value::Bool(false)),
            (
                "paths".into(),
                Value::Array(vec![Value::Text("/tmp/a.txt".into())])
            ),
        ])))
    );

    let save = broker.receive(
        r#"{"kind":"command","command_id":"h-5","command":"dialog.saveFile","payload":{"name":"out.bin"}}"#,
        &mut surface,
    );
    assert_eq!(save.status, Status::Completed);

    let folder = broker.receive(
        r#"{"kind":"command","command_id":"h-6","command":"dialog.pickFolder","payload":null}"#,
        &mut surface,
    );
    assert_eq!(folder.status, Status::Completed);
    assert_eq!(
        folder
            .value
            .as_ref()
            .and_then(|value| value.field("cancelled"))
            .and_then(Value::boolean),
        Some(true)
    );

    assert_eq!(
        broker
            .receive(
                r#"{"kind":"command","command_id":"h-7","command":"window.close"}"#,
                &mut surface,
            )
            .status,
        Status::Completed
    );
    assert!(surface.closed);
}

#[test]
fn codec() {
    let sample = parse(r#"{"text":"hi\n\uD83D\uDE00","list":[1,true,null]}"#).unwrap();
    let encoded = encode(&sample).unwrap();
    assert_eq!(parse(&encoded).unwrap(), sample);
    assert!(parse(r#"{"a":1,"a":2}"#).is_err());
    assert!(parse(&format!("{}{}", "[".repeat(40), "]".repeat(40))).is_err());
    assert!(parse("{\"a\":\"\u{0001}\"}").is_err());

    let reply = Reply {
        receipt: Receipt {
            command: "big-1".into(),
            target: "job.run".into(),
            status: Status::Running,
            value: Some(Value::Text("x".repeat(LIMIT))),
            error: None,
            progress: Some(Update {
                current: 1,
                total: Some(2),
                message: Some("m".repeat(LIMIT + 512)),
            }),
        },
        duplicate: false,
        found: true,
    };
    let wire = frame(&reply);
    assert!(wire.len() <= LIMIT);
    let decoded = parse(&wire).unwrap();
    assert_eq!(
        decoded.field("status").and_then(Value::text),
        Some("running")
    );
    assert_eq!(decoded.field("progress"), Some(&Value::Null));
    assert_eq!(decoded.field("value"), Some(&Value::Null));
}
