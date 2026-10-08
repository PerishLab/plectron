use jsengine::{Engine, Failure, Fault, Invocation, Value};
use jsruntime::{Capabilities, Runtime};
use plectron::resource::Services;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use webengine::resource::{Provider, address};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Host;
impl jsengine::Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}
impl jsruntime::Addons for Host {
    fn install(&mut self, _: &mut Engine, _: &mut Capabilities) -> Result<(), Fault> {
        Ok(())
    }
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plectron-provenance-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join("packages")).unwrap();
        std::fs::write(path.join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn snapshots() {
    let root = Root::new();
    root.write(
        "entry.ts",
        "import {fail} from './failure.ts'; const saved = fail;",
    );
    let first = "export const fail = function (): never { throw 7; };";
    root.write("failure.ts", first);
    let mut services = Services::default();
    let id = services.serve(&root.0).unwrap();
    services.scheme(id, "app://source/").unwrap();
    let url = address("app://source/entry.ts").unwrap();
    let loaded = services.script(&url).unwrap();
    let plain = services.get(&url).unwrap();
    assert_eq!(loaded.response.mime, plain.mime);
    assert_eq!(loaded.response.body, plain.body);
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.script(loaded.source.as_ref().unwrap()).unwrap();
    drop(loaded);
    let callback = runtime.execute("saved;").unwrap();
    let held = runtime.retain(&callback).unwrap();
    let second = format!("type Changed = number;\n{first}");
    root.write("failure.ts", &second);
    let http = address(&format!("{}/entry.ts", services.inspect()[0].address)).unwrap();
    let newer = services.script(&http).unwrap();
    let mut fresh = Runtime::new(Host).unwrap();
    fresh.script(newer.source.as_ref().unwrap()).unwrap();
    services.close(id).unwrap();
    drop(newer);
    runtime.collect().unwrap();
    let failure = runtime
        .call(runtime.held(held).unwrap(), Vec::new())
        .unwrap_err();
    let old = runtime.location(&failure).unwrap();
    assert_eq!(old.resource, "failure.ts");
    assert_eq!(&first[old.span.clone()], "throw 7;");
    assert_eq!(old.precision, jsruntime::Precision::Exact);
    let callback = fresh.execute("saved;").unwrap();
    let failure = fresh.call(callback, Vec::new()).unwrap_err();
    let new = fresh.location(&failure).unwrap();
    assert_eq!(&second[new.span.clone()], "throw 7;");
    assert_ne!(old.span, new.span);
    runtime.release(held).unwrap();
}

#[test]
fn admission() {
    let root = Root::new();
    root.write("entry.ts", "import './bad.ts'; marker = 1;");
    root.write("bad.ts", "export enum Bad { Value }");
    let mut services = Services::default();
    let id = services.serve(&root.0).unwrap();
    services.scheme(id, "app://source/").unwrap();
    let url = address("app://source/entry.ts").unwrap();
    let mut runtime = Runtime::new(Host).unwrap();
    runtime.execute("let marker = 0;").unwrap();
    let rejected = services.script(&url).unwrap();
    assert_eq!(rejected.response.status, 400);
    assert!(rejected.source.is_none());
    assert!(
        String::from_utf8(rejected.response.body)
            .unwrap()
            .contains("bad.ts:")
    );
    assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
    root.write("entry.ts", "marker = 1; import './bad.js';");
    root.write("bad.js", "var forbidden = 1;");
    let rejected = services.script(&url).unwrap().source.unwrap();
    let failure = runtime.script(&rejected).unwrap_err();
    assert_eq!(runtime.location(&failure).unwrap().resource, "bad.js");
    assert_eq!(runtime.execute("marker;").unwrap(), Value::Number(0.0));
}

#[test]
fn application() {
    let mut services = Services::default();
    let id = services.serve(&fixtures::root()).unwrap();
    services.scheme(id, "app://source/").unwrap();
    let mut page = webengine::Page::new(
        "app://source/typescript/index.html",
        domengine::Extent {
            width: 640,
            height: 400,
        },
    )
    .unwrap();
    page.load(&mut services.loader()).unwrap();
    services.close(id).unwrap();
    page.collect().unwrap();
    page.receive("increment".into()).unwrap();
    page.render().unwrap();
    assert_eq!(page.messages(), ["count:1"]);
    let text = std::fs::read_to_string(fixtures::root().join("typescript/model.ts")).unwrap();
    page.receive("failure".into()).unwrap();
    page.render().unwrap();
    let span = "throw \"typescript listener\";";
    let start = text.find(span).unwrap();
    let expected = format!("apps/typescript/model.ts:{start}..{}:", start + span.len());
    assert!(
        page.inspect()
            .events
            .last()
            .unwrap()
            .exception
            .as_ref()
            .unwrap()
            .starts_with(&expected)
    );
    page.receive("asynchronous".into()).unwrap();
    page.render().unwrap();
    let span = "throw \"typescript async\";";
    let start = text.find(span).unwrap();
    let expected = format!("apps/typescript/model.ts:{start}..{}:", start + span.len());
    assert!(page.inspect().jobs.rejected[0].starts_with(&expected));
    page.receive("dispose".into()).unwrap();
    page.render().unwrap();
    page.collect().unwrap();
    page.close();
}

#[test]
fn negotiation() {
    for (mime, body, valid) in [
        ("text/javascript", "throw 1;", true),
        ("application/vnd.plectron.script+json", "{}", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            assert!(
                String::from_utf8(bytes)
                    .unwrap()
                    .contains("Accept: application/vnd.plectron.script+json\r\n")
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let loaded =
            Services::default().script(&address(&format!("http://{endpoint}/entry.ts")).unwrap());
        if valid {
            let loaded = loaded.unwrap();
            assert!(loaded.source.is_none());
            assert_eq!(loaded.response.body, body.as_bytes());
        } else {
            assert!(loaded.is_err());
        }
        worker.join().unwrap();
    }
}

#[test]
fn budget() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/vnd.plectron.script+json\r\nContent-Length: 1048577\r\n\r\n").unwrap();
    });
    let failure = Services::default()
        .script(&address(&format!("http://{endpoint}/entry.ts")).unwrap())
        .err()
        .unwrap();
    assert!(failure.0.contains("byte budget"));
    worker.join().unwrap();
}
