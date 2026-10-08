use base64::Engine;
use plectron_chromium::{artifact, paired};
use project::{Adapters, Target, Workspace};
use serde_json::{Value, json};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps")
}

fn receipts(scripts: &std::collections::BTreeMap<String, Value>) -> Value {
    json!(scripts.iter().map(|(path, script)| json!({
        "path": path, "status": 200, "mime": "text/javascript; charset=utf-8",
        "body": base64::engine::general_purpose::STANDARD.encode(script["body"].as_str().unwrap()),
    })).collect::<Vec<_>>())
}

#[test]
fn canonical() {
    for name in [
        "article",
        "cards",
        "gallery",
        "desk",
        "list",
        "isolation",
        "layers",
    ] {
        let scripts = artifact::scripts(&root(), name).unwrap();
        let verified = artifact::verify(&scripts, &receipts(&scripts)).unwrap();
        let requests = scripts.keys().cloned().collect::<Vec<_>>();
        let resources = paired::resources(&root(), name, "", &requests).unwrap();
        for (path, script) in scripts {
            let mut workspace = Workspace::open(
                root().join(&path),
                Adapters::new(typescript::Typescript, pt::Templates),
            )
            .unwrap();
            let output = workspace.link(Target::Script).unwrap();
            assert_eq!(script["body"], output.javascript);
            assert!(!output.javascript.contains("import "));
            assert!(script["sources"]["packages/engine/src/index.ts"]["sha256"].is_string());
            assert_eq!(
                script["sources"]["packages/engine/src/index.ts"]["kind"],
                "ts"
            );
            assert!(
                script["origins"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|origin| origin["module"] == format!("apps/{path}"))
            );
            for key in ["bytes", "fingerprint"] {
                assert_eq!(verified[&path][key], resources[&path][key]);
            }
        }
    }
}

#[test]
fn rejection() {
    let scripts = artifact::scripts(&root(), "cards").unwrap();
    let base = receipts(&scripts);
    for (key, value) in [
        ("status", json!(404)),
        ("mime", json!("application/octet-stream")),
        ("body", json!("%%%")),
        ("body", json!("eA==")),
        ("path", json!("other.ts")),
    ] {
        let mut changed = base.clone();
        changed[0][key] = value;
        assert!(artifact::verify(&scripts, &changed).is_err(), "{key}");
    }
    assert!(artifact::verify(&scripts, &json!([])).is_err());
    assert!(artifact::verify(&scripts, &json!({})).is_err());
}

#[test]
fn unicode() {
    let scripts = artifact::scripts(&root(), "article").unwrap();
    let verified = artifact::verify(&scripts, &receipts(&scripts)).unwrap();
    let body = scripts["article/main.ts"]["body"].as_str().unwrap();
    assert!(body.contains("café é"));
    assert!(body.contains("中文"));
    assert_eq!(verified["article/main.ts"]["bytes"], body.len());
    assert_ne!(verified["article/main.ts"]["bytes"], body.chars().count());
}

#[test]
fn paths() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("case")).unwrap();
    for source in [
        "<script src='../../outside.ts'></script>",
        "<body>static</body>",
        "<script src='main.ts'></script><script src='./main.ts'></script>",
    ] {
        std::fs::write(directory.path().join("case/index.html"), source).unwrap();
        assert!(artifact::scripts(directory.path(), "case").is_err());
    }
}

#[test]
fn order() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("case")).unwrap();
    std::fs::write(
        directory.path().join("case/index.html"),
        "<script src='z.ts'></script><script src='a.ts'></script>",
    )
    .unwrap();
    assert_eq!(
        artifact::entries(directory.path(), "case").unwrap(),
        ["case/z.ts", "case/a.ts"]
    );
    assert_eq!(
        artifact::entries(&root(), "isolation").unwrap(),
        ["list/main.ts", "list/scenario.js"]
    );
}

#[test]
fn vm() {
    let source = r#"
const fs = require("fs");
const vm = require("vm");
const path = process.argv[1];
const handlers = {};
const target = { on(name, fn) { handlers[name] = fn; }, route: async () => {} };
const context = {
    newPage: async () => target, addInitScript: async () => {},
    newCDPSession: async () => {
        handlers.response({ url: () => "http://localhost/cards/main.ts", status: () => 200,
            headers: () => ({ "content-type": "text/javascript; charset=utf-8" }),
            body: async () => Buffer.from("const value = 1;") });
        throw new Error("ready");
    },
};
const config = { url: "http://localhost/", scenario: "cards", scripts: { "cards/main.ts": "const value = 1;" } };
const source = fs.readFileSync(path, "utf8").replace("__CONFIG__", JSON.stringify(config));
const run = vm.runInNewContext("(" + source + ")");
run({ context: () => ({ browser: () => ({ newContext: async () => context }) }) })
    .then(() => { throw new Error("missing sentinel"); })
    .catch(error => { if (error.message !== "ready") { console.error(error); process.exitCode = 1; } });
"#;
    for name in ["cards.js", "paired.js"] {
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(source)
            .arg(root().parent().unwrap().join("tools/chromium").join(name))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
