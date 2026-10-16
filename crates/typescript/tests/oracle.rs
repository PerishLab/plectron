mod support;

use std::process::Command;

use project::{Kind, Module, Source};

const VERSION: &str = "v24.18.0";
const DRIVER: &str = "const {runInNewContext} = require('node:vm'); const {stripTypeScriptTypes} = require('node:module'); const [source, lowered] = process.argv.slice(1); process.stdout.write(JSON.stringify([runInNewContext(stripTypeScriptTypes(source)), runInNewContext(lowered)]));";

fn compare(text: &str) {
    let source = Source {
        module: Module::new("oracle.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    };
    let lowered = typescript::erase(&source).unwrap();
    let output = Command::new("node")
        .args([
            "--input-type=commonjs",
            "--eval",
            DRIVER,
            "--",
            text,
            &lowered.text,
        ])
        .output()
        .expect("the explicit oracle needs the pinned Node executable");
    assert!(
        output.status.success(),
        "{text}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "[7,7]", "{text}");
}

#[test]
#[ignore = "needs Node v24.18.0 on PATH"]
fn execution() {
    version();
    for text in support::cases::scripts() {
        compare(text);
    }
}

fn version() {
    let version = Command::new("node").arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(String::from_utf8(version.stdout).unwrap().trim(), VERSION);
}

const MODULES: &str = "const {SourceTextModule, createContext} = require('node:vm'); const {stripTypeScriptTypes} = require('node:module'); const {deepStrictEqual} = require('node:assert'); const [source, lowered] = process.argv.slice(1); async function run(text) { const context = createContext({}); const loads = []; const cache = new Map(); const root = new SourceTextModule(text, {context}); await root.link(async specifier => { if (specifier !== './value.ts') throw new Error('unexpected runtime dependency: ' + specifier); if (!cache.has(specifier)) { loads.push(specifier); cache.set(specifier, new SourceTextModule('export const value = 7; export default 7;', {context})); } return cache.get(specifier); }); await root.evaluate(); return {loads, value: root.namespace.result ?? root.namespace.default, keys: Object.keys(root.namespace), namespace: root.namespace.values?.value ?? null}; } (async () => { const results = [await run(stripTypeScriptTypes(source)), await run(lowered)]; deepStrictEqual(results[0], results[1]); process.stdout.write(JSON.stringify(results.map(({loads, value}) => ({loads, value})))); })().catch(error => { console.error(error); process.exitCode = 1; });";

fn module(text: &str, loads: &str) {
    let source = Source {
        module: Module::new("oracle.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    };
    let lowered = typescript::erase(&source).unwrap();
    let output = Command::new("node")
        .args([
            "--experimental-vm-modules",
            "--input-type=commonjs",
            "--eval",
            MODULES,
            "--",
            text,
            &lowered.text,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{text}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = format!("[{{\"loads\":{loads},\"value\":7}},{{\"loads\":{loads},\"value\":7}}]");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        expected,
        "{text}"
    );
}

#[test]
#[ignore = "needs Node v24.18.0 on PATH"]
fn modules() {
    version();
    for text in support::cases::isolated() {
        module(text, "[]");
    }
    for text in support::cases::loaded() {
        module(text, "[\"./value.ts\"]");
    }
}
