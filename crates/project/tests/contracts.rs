use project::{
    Files, Import, Kind, Memory, Module, Package, Provider, Request, Resolve, Resolver, Span,
};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

fn resolver() -> Resolver {
    Resolver::new([Package::parse(
        module("packages/engine"),
        r#"{"name":"@plectron/engine","exports":{".":"./src/index.js","./types":"./src/types.ts"}}"#,
    )
    .unwrap()])
    .unwrap()
}

fn resolve(
    resolver: &Resolver,
    from: &str,
    specifier: &str,
) -> Result<Module, project::Diagnostic> {
    resolver.resolve(Import {
        from: module(from),
        specifier: specifier.into(),
        span: Span::new(7, 19).unwrap(),
    })
}

#[test]
fn identity() {
    assert_eq!(module("src/main.js").kind().unwrap(), Kind::Js);
    assert_eq!(module("src/main.ts").kind().unwrap(), Kind::Ts);
    assert_eq!(module("src/view.pt").kind().unwrap(), Kind::Pt);
    assert!(Module::new("../outside.js").is_err());
    assert!(Module::new("/absolute.js").is_err());
}

#[test]
fn resolution() {
    let resolver = resolver();
    assert_eq!(
        resolve(&resolver, "apps/demo/main.ts", "./view.pt").unwrap(),
        module("apps/demo/view.pt")
    );
    assert_eq!(
        resolve(&resolver, "apps/demo/main.ts", "@plectron/engine").unwrap(),
        module("packages/engine/src/index.js")
    );
    assert_eq!(
        resolve(&resolver, "apps/demo/main.ts", "@plectron/engine/types").unwrap(),
        module("packages/engine/src/types.ts")
    );
    for rejected in [
        "./view",
        "../../../../outside.js",
        "unknown",
        "@plectron/engine/private",
    ] {
        let error = resolve(&resolver, "apps/demo/main.ts", rejected).unwrap_err();
        assert_eq!(error.module, module("apps/demo/main.ts"));
        assert_eq!(error.span, Span::new(7, 19).unwrap());
    }
}

#[test]
fn providers() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(
        directory.path().join("src/main.ts"),
        "export const value = 1;",
    )
    .unwrap();
    let identity = module("src/main.ts");
    let request = Request {
        module: identity.clone(),
    };
    let files = Files::new(directory.path()).unwrap();
    let memory = Memory::new([(identity, "export const value = 1;".into())]);
    assert_eq!(files.read(request.clone()), memory.read(request));

    let missing = Request {
        module: module("src/missing.js"),
    };
    assert_eq!(files.read(missing.clone()), memory.read(missing));
}
