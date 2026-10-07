use project::{Adapter, Adapters, Artifact, Diagnostic, Kind, Module, Source, Span, Unavailable};
use std::cell::RefCell;

struct Recorder<'a>(&'a RefCell<Vec<Kind>>);

impl Adapter for Recorder<'_> {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        self.0.borrow_mut().push(source.kind);
        Ok(Artifact::source(source.module, Kind::Js, source.text))
    }
}

fn source(path: &str) -> Source {
    let module = Module::new(path).unwrap();
    Source {
        kind: module.kind().unwrap(),
        module,
        text: "const result = 1;".into(),
    }
}

#[test]
fn routing() {
    let typescript = RefCell::new(Vec::new());
    let templates = RefCell::new(Vec::new());
    let adapters = Adapters::new(Recorder(&typescript), Recorder(&templates));
    for path in ["main.js", "main.ts", "view.pt"] {
        let source = source(path);
        let artifact = adapters.translate(source.clone()).unwrap();
        assert_eq!(artifact.module, source.module);
        assert_eq!(artifact.text, source.text);
        assert_eq!(artifact.kind, Kind::Js);
    }
    assert_eq!(*typescript.borrow(), vec![Kind::Ts]);
    assert_eq!(*templates.borrow(), vec![Kind::Pt]);
}

#[test]
fn unavailable() {
    let adapters = Adapters::new(Unavailable, Unavailable);
    assert!(adapters.translate(source("main.js")).is_ok());
    for path in ["main.ts", "view.pt"] {
        let source = source(path);
        let failure = adapters.translate(source.clone()).unwrap_err();
        assert_eq!(failure.module, source.module);
        assert_eq!(failure.span, Span::default());
        assert!(failure.message.contains("no adapter"));
    }
}
