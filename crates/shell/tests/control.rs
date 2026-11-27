use platform::shared::{Progress, Region};
use shell::control::json::{LIMIT, parse};
use shell::{
    Addon, Admission, Broker, Context, Host, Ledger, Notice, Outcome, Request, Status, Surface,
    Update, Value,
};

#[derive(Default)]
struct Stub;

impl Surface for Stub {
    fn title(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn drag(&mut self, _: &[Region]) -> Result<(), String> {
        Ok(())
    }

    fn progress(&mut self, _: Progress) -> Result<(), String> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct Worker {
    calls: usize,
    notices: Vec<Notice>,
}

impl Addon for Worker {
    fn namespace(&self) -> &str {
        "job"
    }

    fn execute(&mut self, request: &Request<'_>, _: &mut Context<'_>) -> Outcome {
        self.calls += 1;
        match request.command {
            "job.run" => Outcome::Running(Some(Update {
                current: 1,
                total: Some(3),
                message: Some("started".into()),
            })),
            "job.uncertain" => Outcome::Unknown("transport timeout".into()),
            "job.echo" => Outcome::Completed(request.payload.clone()),
            _ => Outcome::Rejected("unsupported job command".into()),
        }
    }

    fn cancel(&mut self, id: &str, _: &mut Context<'_>) -> Option<Outcome> {
        Some(Outcome::Rejected(format!("cancelled {id}")))
    }

    fn poll(&mut self, _: &mut Context<'_>) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }
}

#[test]
fn ledger() {
    let mut ledger = Ledger::new(2);
    assert_eq!(ledger.capacity(), 2);
    assert!(ledger.empty());
    let first = ledger.admit("cmd-1", "job.run").unwrap();
    assert!(matches!(first, Admission::Execute(_)));
    assert_eq!(ledger.start("cmd-1").unwrap().status, Status::Running);
    assert_eq!(
        ledger
            .uncertain("cmd-1", "network drop".into())
            .unwrap()
            .status,
        Status::Unknown
    );
    assert!(ledger.complete("cmd-1", Value::Null).is_err());
    assert!(ledger.reject("cmd-1", "retry".into()).is_err());
    let dup = ledger.admit("cmd-1", "job.run").unwrap();
    assert!(matches!(dup, Admission::Duplicate(r) if r.status == Status::Unknown));

    assert!(matches!(
        ledger.admit("cmd-2", "job.echo").unwrap(),
        Admission::Execute(_)
    ));
    ledger.start("cmd-2").unwrap();
    ledger.complete("cmd-2", Value::Bool(true)).unwrap();
    assert!(matches!(
        ledger.admit("cmd-3", "job.echo").unwrap(),
        Admission::Execute(_)
    ));
    assert!(ledger.query("cmd-1").is_some());
    assert!(ledger.query("cmd-2").is_none());
    assert_eq!(ledger.count(), 2);

    assert!(matches!(
        ledger.admit("cmd-4", "job.echo").unwrap(),
        Admission::Refused(r) if r.status == Status::Rejected
    ));
    assert!(ledger.query("cmd-1").is_some());
    assert!(ledger.query("cmd-3").is_some());
}

#[test]
fn deduplication() {
    let mut broker = Broker::default();
    broker
        .register(Box::new(Worker {
            calls: 0,
            notices: Vec::new(),
        }))
        .unwrap();
    let mut surface = Stub;

    let first = broker.receive(
        r#"{"kind":"command","command_id":"u-1","command":"job.uncertain","payload":null}"#,
        &mut surface,
    );
    assert_eq!(first.status, Status::Unknown);
    let reply = parse(&broker.pop().unwrap()).unwrap();
    assert_eq!(reply.field("status").and_then(Value::text), Some("unknown"));
    assert_eq!(
        reply.field("duplicate").and_then(Value::boolean),
        Some(false)
    );

    let second = broker.receive(
        r#"{"kind":"command","command_id":"u-1","command":"job.echo","payload":123}"#,
        &mut surface,
    );
    assert_eq!(second.status, Status::Unknown);
    assert_eq!(second.target, "job.uncertain");
    let reply = parse(&broker.pop().unwrap()).unwrap();
    assert_eq!(reply.field("status").and_then(Value::text), Some("unknown"));
    assert_eq!(
        reply.field("duplicate").and_then(Value::boolean),
        Some(true)
    );

    let cancelled = broker.receive(r#"{"kind":"cancel","command_id":"u-1"}"#, &mut surface);
    assert_eq!(cancelled.status, Status::Unknown);
    let _ = broker.pop();
}

#[test]
fn lifecycle() {
    let mut broker = Broker::default();
    broker
        .register(Box::new(Worker {
            calls: 0,
            notices: vec![Notice {
                command: "r-1".into(),
                outcome: Outcome::Completed(Value::Text("done".into())),
            }],
        }))
        .unwrap();
    let mut surface = Stub;

    let started = broker.receive(
        r#"{"kind":"command","command_id":"r-1","command":"job.run"}"#,
        &mut surface,
    );
    assert_eq!(started.status, Status::Running);
    let _ = broker.pop();

    let queried = broker.receive(r#"{"kind":"query","command_id":"r-1"}"#, &mut surface);
    assert_eq!(queried.status, Status::Running);
    let _ = broker.pop();

    broker.poll(&mut surface);
    let reply = parse(&broker.pop().unwrap()).unwrap();
    assert_eq!(
        reply.field("status").and_then(Value::text),
        Some("completed")
    );
    assert_eq!(reply.field("value").and_then(Value::text), Some("done"));

    let running = broker.receive(
        r#"{"kind":"command","command_id":"r-2","command":"job.run"}"#,
        &mut surface,
    );
    assert_eq!(running.status, Status::Running);
    let _ = broker.pop();

    let cancelled = broker.receive(r#"{"kind":"cancel","command_id":"r-2"}"#, &mut surface);
    assert_eq!(cancelled.status, Status::Rejected);
    assert_eq!(cancelled.error.as_deref(), Some("cancelled r-2"));
    let _ = broker.pop();
}

#[test]
fn rejections() {
    let mut broker = Broker::default();
    broker.register(Box::new(Host::new(None))).unwrap();
    broker.allow("window.title");
    let mut surface = Stub;

    let malformed = broker.receive("{not-json", &mut surface);
    assert_eq!(malformed.status, Status::Rejected);
    let _ = broker.pop();

    let oversized = format!(
        r#"{{"kind":"command","command_id":"big","command":"window.title","payload":{{"title":"{}"}}}}"#,
        "a".repeat(LIMIT)
    );
    let rejected = broker.receive(&oversized, &mut surface);
    assert_eq!(rejected.status, Status::Rejected);
    let _ = broker.pop();

    let unauthorized = broker.receive(
        r#"{"kind":"command","command_id":"bad","command":"window.close","payload":{}}"#,
        &mut surface,
    );
    assert_eq!(unauthorized.status, Status::Rejected);
    assert!(unauthorized.error.unwrap().contains("unauthorized"));
    let _ = broker.pop();

    let missing = broker.receive(r#"{"kind":"query","command_id":"none"}"#, &mut surface);
    assert_eq!(missing.status, Status::Rejected);
    let reply = parse(&broker.pop().unwrap()).unwrap();
    assert_eq!(reply.field("found").and_then(Value::boolean), Some(false));

    let allowed = broker.receive(
        r#"{"kind":"command","command_id":"ok","command":"window.title","payload":{"title":"Ok"}}"#,
        &mut surface,
    );
    assert_eq!(allowed.status, Status::Completed);
    let _ = broker.pop();
}
