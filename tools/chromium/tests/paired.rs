use plectron_chromium::compare::{against, projection, validate};
use plectron_chromium::paired::resources;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[test]
fn requests() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    place(root, "pnpm-workspace.yaml", b"packages: ['packages/*']");
    fs::create_dir(root.join("packages")).unwrap();
    place(
        root,
        "gallery/src/index.html",
        b"<script src='main.js'></script>",
    );
    for name in [
        "gallery/src/main.js",
        "gallery/src/style.css",
        "fonts/Regular.ttf",
        "pictures/src/dawn.png",
        "benchmarks/normalize.css",
    ] {
        place(root, name, b"x");
    }
    fs::write(
        root.join("gallery/src/style.css"),
        "src:url('../../fonts/Regular.ttf')",
    )
    .unwrap();
    let result = resources(
        root,
        "gallery",
        "normalized",
        &[
            "gallery/src/index.html".into(),
            "pictures/src/dawn.png".into(),
            "missing.png".into(),
        ],
    )
    .unwrap();
    let names = result.as_object().unwrap();
    assert_eq!(names.len(), 5);
    assert!(names.contains_key("fonts/Regular.ttf"));
    assert!(names.contains_key("pictures/src/dawn.png"));
    assert_eq!(
        names["gallery/src/main.js"]["fingerprint"],
        json!(plectron_chromium::artifact::fingerprint(
            plectron_chromium::artifact::compile(&root.join("gallery/src/main.js")).unwrap()["body"]
                .as_str()
                .unwrap()
                .as_bytes()
        ))
    );
    assert_eq!(names["gallery/src/style.css"]["bytes"], 45);
}

#[test]
fn flow() {
    let value = json!({"boxes": {"region": [24, 196, 680, 384], "record-0": [44, 216, 640, 27]}, "scroll": 48, "extent": 4096});
    let mut changed = value.clone();
    changed["boxes"]["record-0"][1] = json!(224);
    changed["extent"] = json!(3584);
    assert_eq!(
        projection("article", &value),
        projection("article", &changed)
    );
    changed["boxes"]["record-0"][3] = json!(54);
    assert_ne!(
        projection("article", &value),
        projection("article", &changed)
    );

    let native = json!({"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195]}, "scroll": 0, "extent": 24960});
    let browser = json!({"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195.1875]}, "scroll": 0, "extent": 24984});
    assert_eq!(
        projection("gallery", &native),
        projection("gallery", &browser)
    );
}

#[test]
fn freshness() {
    let (native, browser) = reports();
    validate(&native, &browser).unwrap();
    for mutation in [
        "assets",
        "profile",
        "actions",
        "lifecycle",
        "empty",
        "viewport",
    ] {
        let mut changed = native.clone();
        match mutation {
            "assets" => {
                changed["runs"][0]["assets"]["app://scenarios/cards/main.js"]["fingerprint"] =
                    json!(3)
            }
            "profile" => changed["runs"][0]["config"]["scenario.normalized"] = json!(false),
            "actions" => {
                changed["runs"][0]["stages"].as_array_mut().unwrap().pop();
            }
            "lifecycle" => changed["runs"][0]["closed"] = json!(false),
            "empty" => {
                changed["repetitions"] = json!(0);
                changed["runs"] = json!([]);
            }
            "viewport" => changed["case"]["viewport"] = json!([640, 480]),
            _ => unreachable!(),
        }
        assert!(validate(&changed, &browser).is_err(), "mutation {mutation}");
    }

    let different = json!({"different": true});
    for mutation in ["none", "stale", "oracle", "geometry"] {
        let (native, mut browser) = reports();
        match mutation {
            "stale" => {
                browser["runs"][0]["stages"][2]["file"] = json!("seed");
                browser["runs"][0]["stages"][2]["oracle"] = json!("seed");
            }
            "oracle" => browser["runs"][0]["stages"][2]["oracle"] = json!("seed"),
            "geometry" => browser["runs"][0]["stages"][2]["state"]["geometry"] = different.clone(),
            _ => {}
        }
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("native.json"),
            serde_json::to_vec(&native).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join("report.json"),
            serde_json::to_vec(&browser).unwrap(),
        )
        .unwrap();
        let result = against(
            &directory.path().join("native.json"),
            directory.path(),
            |path| Ok(path.file_name().unwrap().to_string_lossy().into()),
        )
        .unwrap();
        assert_eq!(
            result["passed"],
            json!(mutation == "none"),
            "mutation {mutation}"
        );
        assert!(result["ratio"].is_null());
    }
}

fn reports() -> (Value, Value) {
    let resources = json!({
        "cards/main.js": {"bytes": 1, "fingerprint": 2},
        "cards/src/style.css": {"bytes": 1, "fingerprint": 2},
        "fonts/Lato-Regular.ttf": {"bytes": 1, "fingerprint": 2},
        "benchmarks/normalize.css": {"bytes": 1, "fingerprint": 2},
    });
    let geometry = json!({"boxes": {"region": [20, 168, 680, 320]}, "scroll": 0, "extent": 640});
    let stages = ["seed", "steady", "edit"].map(|action| json!({
        "action": action, "records": 8, "geometry": geometry,
        "capture": {"file": if action == "steady" { "seed" } else { action }, "ready": 1_000_000},
    }));
    let native = json!({
        "debug": false, "case": {"id": "cards", "entry": "cards/src/index.html", "actions": ["edit"], "viewport": [720, 640]},
        "count": 8, "edits": 1, "repetitions": 1, "capture": "native", "runs": [{
            "failure": null, "closed": true,
            "config": {"scenario.capture": true, "scenario.normalized": true},
            "assets": resources.as_object().unwrap().iter().map(|(key, value)| (format!("app://scenarios/{key}"), value.clone())).collect::<serde_json::Map<_, _>>(),
            "stages": stages,
        }],
    });
    let stages = ["seed", "steady", "edit"].map(|action| {
        json!({
            "action": action,
            "file": if action == "steady" { "seed" } else { action },
            "oracle": if action == "steady" { "seed" } else { action },
            "ready": 1,
            "state": {"records": 8, "geometry": geometry},
            "work": {},
        })
    });
    let browser = json!({
        "config": {"scenario": "cards", "count": 8, "runs": 1}, "errors": [],
        "resources": resources, "endpoint": "browser", "runs": [{"index": 0, "stages": stages}],
    });
    (native, browser)
}

fn place(root: &Path, name: &str, data: &[u8]) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}
