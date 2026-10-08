use std::path::Path;
use std::process::Command;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile(source: &str, template: &str) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("view.d.pt.ts"),
        pt::compile(template).unwrap().shape.declaration(),
    )
    .unwrap();
    std::fs::write(directory.path().join("main.ts"), source).unwrap();
    std::fs::write(directory.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"strict":true,"noEmit":true,"module":"ESNext","moduleResolution":"Bundler","allowArbitraryExtensions":true,"types":[]},"include":["*.ts"]}"#).unwrap();
    Command::new("pnpm")
        .args(["exec", "tsc", "--project"])
        .arg(directory.path().join("tsconfig.json"))
        .current_dir(root())
        .output()
        .unwrap()
}

#[test]
fn projection() {
    let root = root();
    let projection = plectron_check::types::project(&root).unwrap();
    assert_eq!(projection.templates, 10);
    let path = projection.root().join("apps/typescript/view.d.pt.ts");
    let declaration = std::fs::read_to_string(path).unwrap();
    assert!(declaration.contains("increment:") && declaration.contains("count:"));
    assert!(!projection.root().join("types/pt.d.ts").exists());
    assert!(!projection.root().join("apps/typescript/view.js").exists());
    assert_eq!(
        std::fs::read(projection.root().join("apps/typescript/main.ts")).unwrap(),
        std::fs::read(root.join("apps/typescript/main.ts")).unwrap()
    );
}

#[test]
fn structure() {
    let template = "<template tag=\"div\" @click={activate}><button ref=\"action\"></button><text ref=\"value\"/></template>";
    let prefix = "import Template from './view.pt'; const document = {createElement: (tag: string) => ({tag}), createTextNode: (text: string) => ({text})}; const view = Template(document); ";
    let output = compile(
        &format!(
            "{prefix}view.host.tag; view.refs.action.tag; view.refs.value.text; view.listen({{activate: () => {{}}}}, {{}});"
        ),
        template,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    for (source, diagnostic) in [
        ("view.refs.missing;", "missing"),
        ("view.refs.value.tag;", "tag"),
        ("view.refs.action.text;", "text"),
        ("view.listen({}, {});", "activate"),
        ("view.listen({wrong: () => {}}, {});", "wrong"),
        ("view.dispose();", "dispose"),
    ] {
        let output = compile(&format!("{prefix}{source}"), template);
        assert!(!output.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&output.stdout).contains(diagnostic));
    }
}

#[test]
fn absence() {
    let template = "<template tag=\"div\"><span ref=\"body\"></span></template>";
    let prefix = "import Template from './view.pt'; const document = {createElement: (tag: string) => ({tag})}; const view = Template(document); ";
    let output = compile(&format!("{prefix}view.refs.body.tag;"), template);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let output = compile(&format!("{prefix}view.listen({{}}, {{}});"), template);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("listen"));
}

#[test]
fn parallel() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["apps", "packages", "types"] {
        std::fs::create_dir(directory.path().join(name)).unwrap();
    }
    std::fs::write(
        directory.path().join("apps/view.pt"),
        "<template tag=\"div\"></template>",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("apps/view.d.pt.ts"),
        "export default function Template(): any;",
    )
    .unwrap();
    let failure = plectron_check::types::project(directory.path())
        .err()
        .unwrap();
    assert!(failure.contains("parallel template declaration"));
}
