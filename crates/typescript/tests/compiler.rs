mod support;

use std::process::Command;

use project::{Kind, Module, Source};

const DRIVER: &str = r#"
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const {runInNewContext, SourceTextModule, createContext} = require('node:vm');
const metadata = require('typescript/package.json');
assert.equal(process.version, 'v24.18.0');
assert.equal(metadata.version, '7.0.2');
assert.equal(metadata.gitHead, '2bd066d87f5bafd315be9f40889d0a60b9e58e0b');
const compiler = path.join(path.dirname(require.resolve('typescript/package.json')), 'bin/tsc');
const version = spawnSync(process.execPath, [compiler, '--version'], {encoding: 'utf8', timeout: 10000});
assert.equal(version.status, 0, version.stderr);
assert.equal(version.stdout.trim(), 'Version 7.0.2');
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'plectron-tsc-'));
const inputs = [];
const cases = [];
const arguments = process.argv.slice(1);
for (let index = 0; index < arguments.length; index += 3) {
    const [kind, source, lowered] = arguments.slice(index, index + 3);
    const name = `case${index / 3}`;
    const input = path.join(directory, `${name}.ts`);
    fs.writeFileSync(input, source);
    inputs.push(input);
    cases.push({kind, source, lowered, output: path.join(directory, 'out', `${name}.js`)});
}
async function evaluate(text) {
    const context = createContext({});
    const loads = [];
    const cache = new Map();
    const root = new SourceTextModule(text, {context});
    await root.link(async specifier => {
        assert.equal(specifier, './value.ts');
        if (!cache.has(specifier)) {
            loads.push(specifier);
            cache.set(specifier, new SourceTextModule('export const value = 7; export default 7;', {context}));
        }
        return cache.get(specifier);
    });
    await root.evaluate();
    return {loads, value: root.namespace.result ?? root.namespace.default,
        keys: Object.keys(root.namespace), namespace: root.namespace.values?.value ?? null};
}
(async () => {
    const emitted = spawnSync(process.execPath, [compiler, '--ignoreConfig', '--noCheck', '--noResolve',
        '--lib', 'esnext', '--target', 'esnext', '--module', 'preserve', '--verbatimModuleSyntax',
        '--useDefineForClassFields', 'true', '--moduleDetection', 'legacy',
        '--pretty', 'false', '--outDir', path.join(directory, 'out'), ...inputs],
        {encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024});
    assert.equal(emitted.status, 0, `${emitted.stdout}\n${emitted.stderr}\n${emitted.error ?? ''}`);
    for (const entry of cases) {
        const compiled = fs.readFileSync(entry.output, 'utf8');
        try {
            if (entry.kind === 'script') {
                assert.equal(runInNewContext(compiled), 7);
                assert.equal(runInNewContext(entry.lowered), 7);
            } else {
                const reference = await evaluate(compiled);
                const lowered = await evaluate(entry.lowered);
                assert.deepEqual(lowered, reference);
                assert.equal(reference.value, 7);
                assert.deepEqual(reference.loads, entry.kind === 'loaded' ? ['./value.ts'] : []);
            }
        } catch (error) {
            error.message += `\nsource: ${entry.source}\ncompiled: ${compiled}\nlowered: ${entry.lowered}`;
            throw error;
        }
    }
    fs.rmSync(directory, {recursive: true});
    process.stdout.write(`verified ${cases.length}`);
})().catch(error => {
    console.error(error);
    console.error(`retained compiler evidence: ${directory}`);
    process.exitCode = 1;
});
"#;

#[test]
#[ignore = "needs Node and the typescript npm package"]
fn execution() {
    let mut command = Command::new("node");
    command.args([
        "--experimental-vm-modules",
        "--input-type=commonjs",
        "--eval",
        DRIVER,
        "--",
    ]);
    let groups = [
        ("script", support::cases::scripts()),
        ("isolated", support::cases::isolated()),
        ("loaded", support::cases::loaded()),
    ];
    for (kind, cases) in groups {
        for text in cases {
            let source = Source {
                module: Module::new("oracle.ts").unwrap(),
                kind: Kind::Ts,
                text: (*text).into(),
            };
            let lowered = typescript::erase(&source).unwrap();
            command.args([kind, text, &lowered.text]);
        }
    }
    let output = command
        .output()
        .expect("the explicit compiler oracle needs pinned Node and pnpm dependencies");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "verified 72");
}
