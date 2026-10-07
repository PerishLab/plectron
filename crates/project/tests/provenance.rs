use project::{
    Adapter, Artifact, Diagnostic, Javascript, Kind, Linker, Mapping, Memory, Module, Origin,
    Resolver, Source, Span, Target,
};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

fn occurrence(text: &str, token: &str) -> Span {
    let start = text.find(token).unwrap();
    Span {
        start,
        end: start + token.len(),
    }
}

#[test]
fn linking() {
    let entry = "import { value } from \"./data.js\";\nexport const result = value + 1;\nexport { result as answer };";
    let data = "export const value = \"雪\";";
    for target in [Target::Module, Target::Script] {
        let memory = Memory::new([
            (module("main.js"), entry.into()),
            (module("data.js"), data.into()),
        ]);
        let output = Linker::new(memory, Resolver::default(), Javascript)
            .link(module("main.js"), target)
            .unwrap();
        for (path, original, token) in
            [("main.js", entry, "value + 1"), ("data.js", data, "\"雪\"")]
        {
            let origin = output
                .origin(occurrence(&output.javascript, token))
                .unwrap();
            assert_eq!(origin.module, module(path));
            assert_eq!(origin.source, occurrence(original, token));
            assert_eq!(origin.mapping, Mapping::Exact);
        }
        for token in ["const module_", "return {"] {
            let span = occurrence(&output.javascript, token);
            assert!(output.origin(span).is_none());
        }
        assert!(
            output
                .origin(occurrence(&output.javascript, "const value = module_"))
                .is_none()
        );
    }
}

#[test]
fn replacement() {
    let source = "export default function create() { return 7; }";
    let memory = Memory::new([(module("main.js"), source.into())]);
    let output = Linker::new(memory, Resolver::default(), Javascript)
        .link(module("main.js"), Target::Module)
        .unwrap();
    let origin = output
        .origin(occurrence(&output.javascript, "const create = function"))
        .unwrap();
    assert_eq!(origin.mapping, Mapping::Range);
    assert_eq!(
        origin.source,
        occurrence(source, "export default function create")
    );
    let origin = output
        .origin(occurrence(&output.javascript, "return 7"))
        .unwrap();
    assert_eq!(origin.mapping, Mapping::Exact);
    assert_eq!(origin.source, occurrence(source, "return 7"));
}

#[test]
fn points() {
    let source = "const value = 1;";
    for target in [Target::Module, Target::Script] {
        let memory = Memory::new([(module("main.js"), source.into())]);
        let output = Linker::new(memory, Resolver::default(), Javascript)
            .link(module("main.js"), target)
            .unwrap();
        let body = occurrence(&output.javascript, source);
        let start = output
            .origin(Span {
                start: body.start,
                end: body.start,
            })
            .unwrap();
        assert_eq!(start.source, Span::default());
        let end = Span {
            start: body.end,
            end: body.end,
        };
        assert!(output.origin(end).is_none());
        assert!(
            output
                .origin(Span {
                    start: body.end,
                    end: body.start
                })
                .is_none()
        );
    }
}

struct Generated;

impl Adapter for Generated {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        let text = "export const coarse = 1;\nconst exact = 2;".to_string();
        let boundary = text.find("const exact").unwrap();
        Ok(Artifact {
            module: source.module.clone(),
            kind: Kind::Js,
            origins: vec![
                Origin {
                    mapping: Mapping::Range,
                    generated: Span {
                        start: 0,
                        end: boundary,
                    },
                    module: source.module.clone(),
                    source: Span {
                        start: 0,
                        end: boundary,
                    },
                },
                Origin {
                    mapping: Mapping::Exact,
                    generated: Span {
                        start: boundary,
                        end: text.len(),
                    },
                    module: source.module,
                    source: Span {
                        start: 40,
                        end: 40 + text.len() - boundary,
                    },
                },
            ],
            text,
        })
    }
}

#[test]
fn composition() {
    let memory = Memory::new([(module("main.ts"), "original source".into())]);
    let output = Linker::new(memory, Resolver::default(), Generated)
        .link(module("main.ts"), Target::Module)
        .unwrap();
    let origin = output
        .origin(occurrence(&output.javascript, "coarse"))
        .unwrap();
    assert_eq!(origin.mapping, Mapping::Range);
    assert_eq!(origin.source, Span { start: 0, end: 25 });
    let origin = output
        .origin(occurrence(&output.javascript, "exact"))
        .unwrap();
    assert_eq!(origin.mapping, Mapping::Exact);
    assert_eq!(origin.source, Span { start: 46, end: 51 });
    assert_eq!(origin.module, module("main.ts"));
}

struct Prepared(Artifact);

impl Adapter for Prepared {
    fn translate(&self, _: Source) -> Result<Artifact, Diagnostic> {
        Ok(self.0.clone())
    }
}

#[test]
fn unmapped() {
    let mut artifact = Artifact::source(
        module("main.js"),
        Kind::Js,
        "export default function create() { return 7; }".into(),
    );
    artifact.origins.clear();
    let memory = Memory::new([(module("main.js"), artifact.text.clone())]);
    let output = Linker::new(memory, Resolver::default(), Prepared(artifact))
        .link(module("main.js"), Target::Module)
        .unwrap();
    assert!(output.origins.is_empty());
    assert!(
        output
            .origin(occurrence(&output.javascript, "return 7"))
            .is_none()
    );
}

#[test]
fn invalid() {
    let artifact = Artifact::source(module("main.js"), Kind::Js, "const value = 1;".into());
    let mut uneven = artifact.clone();
    uneven.origins[0].source.end += 1;
    let mut outside = artifact.clone();
    outside.origins[0].generated.end += 1;
    let mut overlap = artifact.clone();
    overlap.origins.push(overlap.origins[0].clone());
    let mut identity = artifact.clone();
    identity.module = module("other.js");
    let mut kind = artifact.clone();
    kind.kind = Kind::Ts;
    for invalid in [uneven, outside, overlap, identity, kind] {
        let memory = Memory::new([(module("main.js"), artifact.text.clone())]);
        let failure = Linker::new(memory, Resolver::default(), Prepared(invalid))
            .link(module("main.js"), Target::Module)
            .unwrap_err();
        assert_eq!(failure.module, module("main.js"));
        assert!(
            failure.message.starts_with("invalid artifact")
                || failure.message.starts_with("adapter must")
        );
    }
}
