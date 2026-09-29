use project::{Kind, Module, Source, parse};

fn source(text: &str) -> Source {
    Source {
        module: Module::new("src/main.js").unwrap(),
        kind: Kind::Js,
        text: text.into(),
    }
}

#[test]
fn structure() {
    let source = source(
        r#"
const description = "import { hidden } from './ignored.js';";
/* declarations are formatting independent */
import {
    value as input,
    other
} from "./data.js";
import "./effect.js";
export
const result = input;
export { result as answer, other };
"#,
    );
    let syntax = parse(&source).unwrap();
    assert_eq!(syntax.dependencies.len(), 2);
    assert_eq!(syntax.dependencies[0].specifier, "./data.js");
    assert_eq!(syntax.dependencies[1].specifier, "./effect.js");
    assert_eq!(syntax.bindings.len(), 2);
    assert_eq!(syntax.bindings[0].imported, "value");
    assert_eq!(syntax.bindings[0].local, "input");
    assert_eq!(syntax.exports.len(), 3);
    assert_eq!(syntax.exports[0].exported, "result");
    assert_eq!(syntax.exports[1].exported, "answer");
    assert_eq!(syntax.exports[2].local, "other");
    assert!(!syntax.body.is_empty());
    let rendered = syntax.render(&source.text);
    assert!(rendered.contains("const description"));
    assert!(rendered.contains("const result = input;"));
    assert!(!rendered.contains("from \"./data.js\""));
    assert!(!rendered.contains("export {"));
}

#[test]
fn functions() {
    let source = source(
        r#"import value from "./value.js";
export default function create() {
    return value;
}"#,
    );
    let syntax = parse(&source).unwrap();
    assert_eq!(syntax.bindings[0].imported, "default");
    assert_eq!(syntax.exports[0].exported, "default");
    assert_eq!(syntax.exports[0].local, "create");
    assert!(
        syntax
            .render(&source.text)
            .contains("const create = function()")
    );
}

#[test]
fn rejection() {
    for text in [
        "import * as values from \"./data.js\";",
        "import { value } from \"./data.js\"",
        "export let value = 1;",
        "/* unfinished",
    ] {
        let source = source(text);
        let error = parse(&source).unwrap_err();
        assert_eq!(error.module, source.module);
        assert!(error.span.end >= error.span.start);
        assert!(!error.message.is_empty());
    }
}
