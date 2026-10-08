use super::*;
use project::{Adapters, Javascript, Linker, Memory, Resolver, Target};

struct Host;
impl jsengine::Addons for Host {
    fn call(
        &mut self,
        _: &mut jsengine::Engine,
        _: usize,
        _: jsengine::Invocation,
    ) -> Result<jsengine::Value, jsengine::Failure> {
        unreachable!()
    }
}
impl jsruntime::Addons for Host {
    fn install(
        &mut self,
        _: &mut jsengine::Engine,
        _: &mut jsruntime::Capabilities,
    ) -> Result<(), jsengine::Fault> {
        Ok(())
    }
}

fn output(text: &str) -> Output {
    let module = Module::new("entry.ts").unwrap();
    Linker::new(
        Memory::new([(module.clone(), text.into())]),
        Resolver::new([]).unwrap(),
        Adapters::new(typescript::Typescript, Javascript),
    )
    .link(module, Target::Script)
    .unwrap()
}

#[test]
fn roundtrip() {
    let output = output("const value: number = 1; throw value;");
    let original = output.origins.clone();
    let text = output.javascript.clone();
    let response = encode(output).unwrap();
    let payload: Envelope = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(payload.modules, ["entry.ts"]);
    let decoded = decode(response).unwrap();
    assert_eq!(decoded.response.mime, "text/javascript");
    assert_eq!(decoded.response.body, text.as_bytes());
    let script = decoded.source.unwrap();
    assert_eq!(script.source().text(), text);
    let mut runtime = jsruntime::Runtime::new(Host).unwrap();
    let failure = runtime.script(&script).unwrap_err();
    let site = failure.site().unwrap();
    let expected = project::locate(
        &original,
        Span {
            start: site.span.start,
            end: site.span.end,
        },
    )
    .unwrap();
    let actual = runtime.location(&failure).unwrap();
    assert_eq!(actual.resource, expected.module.path());
    assert_eq!(actual.span, expected.source.start..expected.source.end);
    assert_eq!(
        actual.precision == jsruntime::Precision::Exact,
        expected.mapping == Mapping::Exact
    );
}

#[test]
fn malformed() {
    let base = serde_json::json!({
        "version": 1, "javascript": "𐐀;", "modules": ["entry.ts"],
        "origins": [{"module": 0, "generated": [0, 4], "source": [2, 6], "exact": true}]
    });
    let mut cases = Vec::new();
    for (key, value) in [
        ("/version", serde_json::json!(2)),
        ("/modules", serde_json::json!(["../entry.ts"])),
        ("/origins/0/module", serde_json::json!(1)),
        ("/origins/0/generated", serde_json::json!([0, 1])),
        ("/origins/0/generated", serde_json::json!([0, 6])),
        ("/origins/0/generated", serde_json::json!([4, 0])),
        ("/origins/0/source", serde_json::json!([6, 2])),
        ("/origins/0/source", serde_json::json!([0, 1])),
        (
            "/origins",
            serde_json::json!([base["origins"][0].clone(), base["origins"][0].clone()]),
        ),
        (
            "/javascript",
            serde_json::json!("x".repeat(Kind::Text.limit() + 1)),
        ),
    ] {
        let mut payload = base.clone();
        *payload.pointer_mut(key).unwrap() = value;
        cases.push(serde_json::to_vec(&payload).unwrap());
    }
    let mut extra = base.clone();
    extra["extra"] = serde_json::json!(true);
    cases.push(serde_json::to_vec(&extra).unwrap());
    cases.extend([vec![0xff], vec![b' '; LIMIT + 1]]);
    for body in cases {
        assert!(
            decode(Response {
                status: 200,
                mime: MIME.into(),
                body
            })
            .is_err()
        );
    }
    assert!(
        decode(Response {
            status: 404,
            mime: MIME.into(),
            body: serde_json::to_vec(&base).unwrap()
        })
        .is_err()
    );
}

#[test]
fn fallback() {
    let decoded = decode(Response {
        status: 404,
        mime: "text/javascript".into(),
        body: b"missing".to_vec(),
    })
    .unwrap();
    assert_eq!(decoded.response.status, 404);
    assert_eq!(decoded.response.body, b"missing");
    assert!(decoded.source.is_none());
    assert!(
        decode(Response {
            status: 200,
            mime: "text/javascript".into(),
            body: vec![b'x'; Kind::Text.limit() + 1]
        })
        .is_err()
    );
}

#[test]
fn workloads() {
    for name in ["gallery", "chat", "cards", "layers", "typescript"] {
        let application = fixtures::Application::new(name);
        let entry = if matches!(name, "typescript" | "chat") {
            "/main.ts"
        } else {
            "/main.js"
        };
        let output = application.load(entry).unwrap().script.unwrap();
        let code = output.javascript.len();
        let entries = output.origins.len();
        let response = encode(output).unwrap();
        eprintln!(
            "script transport {name}: code={code} mappings={entries} wire={} limit={LIMIT}",
            response.body.len()
        );
        assert!(code <= Kind::Text.limit());
        assert!(response.body.len() <= LIMIT);
        assert!(decode(response).unwrap().source.is_some());
    }
}

#[test]
fn budgets() {
    let payload = Envelope {
        version: 1,
        javascript: "x".repeat(Kind::Text.limit()),
        modules: vec!["entry.ts".into()],
        origins: vec![Segment {
            module: 0,
            generated: [0, Kind::Text.limit()],
            source: [0, Kind::Text.limit()],
            exact: true,
        }],
    };
    let body = serde_json::to_vec(&payload).unwrap();
    assert!(body.len() > Kind::Text.limit());
    let decoded = decode(Response {
        status: 200,
        mime: MIME.into(),
        body,
    })
    .unwrap();
    assert_eq!(decoded.response.body.len(), Kind::Text.limit());
    let mut oversized = output("const value = 1;");
    oversized.javascript = "x".repeat(Kind::Text.limit() + 1);
    assert!(encode(oversized).is_err());
    let mut oversized = output("const value = 1;");
    oversized.origins[0].module = Module::new(format!("{}.ts", "x".repeat(LIMIT))).unwrap();
    assert!(encode(oversized).is_err());
}
