use jsengine::{Addons, Engine, Failure, Value};
use pt::{Fault, bundle, compile, script};

struct Bare;

impl Addons for Bare {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        panic!("component lifecycle should not call native addons")
    }
}

#[test]
fn lifecycle() {
    let app = "<template tag=\"div\"><span>parent</span></template>";
    let child = "<template tag=\"span\">child</template>";
    let generated = compile(app).unwrap().javascript;
    assert!(generated.contains("export default function Template(document)"));
    assert!(!generated.contains("target"));
    assert!(!generated.contains("dispose"));
    let artifact = bundle(
        "main.js",
        |path| match path {
            "main.js" => Ok(r#"
import App from "./app.pt";
import Child from "./child.pt";
const node = (name) => ({
    name: name,
    children: [],
    appendChild(child) { this.children[this.children.length] = child; },
    removeChild(child) {
        const retained = [];
        for (const entry of this.children) {
            if (entry !== child) { retained[retained.length] = entry; }
        }
        this.children = retained;
    },
    setText(value) { this.text = value; }
});
const document = { createElement: node, createTextNode: (value) => ({text: value}) };
const target = node("target");
const first = App(document);
const second = App(document);
const child = Child(document);
const detached = target.children.length === 0;
first.host.appendChild(child.host);
target.appendChild(first.host);
target.appendChild(second.host);
const inserted = target.children.length === 2
    && target.children[0] !== target.children[1]
    && target.children[0].children.length === 2;
first.host.removeChild(child.host);
target.removeChild(first.host);
const retained = target.children.length === 1
    && target.children[0].children.length === 1;
target.removeChild(second.host);
export const result = detached && inserted && retained
    && target.children.length === 0;
"#
            .into()),
            "app.pt" => Ok(app.into()),
            "child.pt" => Ok(child.into()),
            _ => Err(Fault("unknown module".into())),
        },
        |_, specifier| Ok(specifier.trim_start_matches("./").into()),
    )
    .unwrap();
    let mut engine = Engine::default();
    assert_eq!(
        engine
            .execute(&(artifact.javascript + "\nmodule2.result;"), &mut Bare)
            .unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn component() {
    let source = include_str!("../../../packages/components/src/input.pt");
    let artifact = compile(source).unwrap();
    assert!(
        artifact
            .javascript
            .contains("export default function Template(document)")
    );
    assert!(artifact.javascript.contains("refs.value = part0"));
    assert!(artifact.javascript.contains("refs.caret = part1"));
    assert!(
        artifact
            .javascript
            .contains("return {host: host, refs: refs}")
    );
    assert!(!artifact.javascript.contains("attachEditor"));
    assert!(compile("<template tag=\"p-input\"></template>").is_err());
    assert!(compile("<template tag=\"TodoRow\"></template>").is_err());
    assert!(compile("<template tag=\"div\"><p-caret/></template>").is_err());
    assert!(compile("<template tag=\"div\"><TodoRow/></template>").is_err());
    assert!(compile("<template tag=\"div\"><Input/></template>").is_err());
    assert!(compile("<template tag=\"div\"><unknown/></template>").is_err());
    assert!(compile("<template tag=\"div\">before<span/></template>").is_ok());
    assert!(compile("<template tag=\"div\"><span/>after</template>").is_ok());
    assert!(compile("<template tag=\"div\">{value}</template>").is_err());
    assert!(compile("<template tag=\"div\"><a href=\"{url}\"/></template>").is_err());
    assert!(
        compile("<template tag=\"div\"><span ref=\"same\"/><span ref=\"same\"/></template>")
            .is_err()
    );
    assert!(compile("<template tag=\"div\"><span id=\"a\" id=\"b\"/></template>").is_err());
    assert!(compile("<template tag=\"div\"><span style=\"color: red\"/></template>").is_err());
    assert!(compile("<template tag=\"div\"><button @click=\"toggle\"/></template>").is_err());
    assert!(compile("<template tag=\"div\"><button @click={toggle()}/></template>").is_err());
    assert!(
        compile("<template tag=\"div\"><button @click={toggle} @click={other}/></template>")
            .is_err()
    );
    assert!(compile("<template tag=\"div\"><img></img></template>").is_err());
    assert!(compile("<template tag=\"img\"><span/></template>").is_err());
    assert!(compile("<template tag=\"div\"><text ref=\"bad-name\"/></template>").is_err());
    assert!(compile("<template tag=\"div\"><span></template>").is_err());
    assert!(compile("<script>let value = 1;</script><template tag=\"div\"></template>").is_err());
    assert!(compile("<script>var value = 1;</script><template tag=\"div\"></template>").is_err());
    assert!(
        compile(
            "import { create as Input } from \"./input.pt\";\n<template tag=\"div\"></template>"
        )
        .is_err()
    );
    assert!(compile("<template tag=\"div\"></template><style>div {color: red;}</style>").is_err());
    assert!(compile("<template tag=\"div\" mount=\"app\"></template>").is_err());
}

#[test]
fn markup() {
    let template = "<template tag=\"div\" id=\"frame\">\n  <span ref=\"label\" dir=\"rtl\" class=\"x\"> hi </span>before<strong ref=\"bold\">中</strong>after<a href=\"/next\" rel=\"next\">link</a><img src=\"pic\"/>\n</template>";
    let artifact = bundle(
        "main.js",
        |path| match path {
            "main.js" => Ok(r#"
import Template from "./view.pt";
const node = (name) => ({
    name: name,
    children: [],
    attributes: {},
    appendChild(child) { this.children[this.children.length] = child; },
    setAttribute(name, value) { this.attributes[name] = value; }
});
const document = {createElement: node, createTextNode: (value) => ({text: value})};
const view = Template(document);
const children = view.host.children;
export const result = view.host.attributes.id === "frame"
    && children.length === 6
    && children[0] === view.refs.label
    && children[0].attributes.dir === "rtl"
    && children[0].attributes.class === "x"
    && children[0].children[0].text === " hi "
    && children[1].text === "before"
    && children[2] === view.refs.bold
    && children[2].children[0].text === "中"
    && children[3].text === "after"
    && children[4].attributes.href === "/next"
    && children[4].attributes.rel === "next"
    && children[5].attributes.src === "pic";
"#
            .into()),
            "view.pt" => Ok(template.into()),
            _ => Err(Fault("unknown module".into())),
        },
        |_, specifier| Ok(specifier.trim_start_matches("./").into()),
    )
    .unwrap();
    let mut engine = Engine::default();
    assert_eq!(
        engine
            .execute(&(artifact.javascript + "\nmodule1.result;"), &mut Bare)
            .unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn events() {
    let template = "<template tag=\"div\" @pointerenter={enter}><button ref=\"button\" @click={toggle}>Go</button></template>";
    let artifact = bundle(
        "main.js",
        |path| match path {
            "main.js" => Ok(r#"
import Template from "./view.pt";
const node = (name) => ({
    name: name,
    children: [],
    listeners: [],
    appendChild(child) { this.children[this.children.length] = child; },
    addEventListener(kind, handler, options) {
        this.listeners[this.listeners.length] = {kind: kind, handler: handler, scope: options.scope};
    }
});
const document = {createElement: node, createTextNode: (value) => ({text: value})};
const view = Template(document);
const detached = view.host.listeners.length === 0 && view.refs.button.listeners.length === 0;
let calls = "";
const enter = () => { calls += "e"; };
const toggle = () => { calls += "t"; };
const scope = {id: 1};
view.listen({enter: enter, toggle: toggle}, scope);
const root = view.host.listeners[0];
const child = view.refs.button.listeners[0];
root.handler();
child.handler();
export const result = detached
    && view.host.listeners.length === 1
    && view.refs.button.listeners.length === 1
    && root.kind === "pointerenter" && root.scope === scope
    && child.kind === "click" && child.scope === scope
    && calls === "et";
"#
            .into()),
            "view.pt" => Ok(template.into()),
            _ => Err(Fault("unknown module".into())),
        },
        |_, specifier| Ok(specifier.trim_start_matches("./").into()),
    )
    .unwrap();
    let mut engine = Engine::default();
    assert_eq!(
        engine
            .execute(&(artifact.javascript + "\nmodule1.result;"), &mut Bare)
            .unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn modules() {
    let artifact = bundle(
        "app.js",
        |path| match path {
            "app.js" => Ok("import { value } from \"./data.js\";\nconst result = value;".into()),
            "data.js" => Ok("export const value = 7;".into()),
            _ => Err(Fault("unknown module".into())),
        },
        |_, specifier| Ok(specifier.trim_start_matches("./").into()),
    )
    .unwrap();
    assert!(artifact.javascript.contains("const module0"));
    assert!(artifact.javascript.contains("const value = module0.value;"));
    assert!(!artifact.javascript.contains("import "));
    let cycle = bundle(
        "app.js",
        |_| Ok("import \"./app.js\";".into()),
        |_, _| Ok("app.js".into()),
    );
    assert!(cycle.is_err());
}

#[test]
fn classic() {
    let artifact = script(
        "app.js",
        |path| match path {
            "app.js" => Ok("import { value } from \"./data.js\";\nlet result = value + 1;".into()),
            "data.js" => Ok("export const value = 7;".into()),
            _ => Err(Fault("unknown module".into())),
        },
        |_, specifier| Ok(specifier.trim_start_matches("./").into()),
    )
    .unwrap();
    let mut engine = Engine::default();
    engine.execute(&artifact.javascript, &mut Bare).unwrap();
    assert_eq!(
        engine.execute("result;", &mut Bare).unwrap(),
        Value::Number(8.0)
    );
    assert!(!artifact.javascript.contains("import "));
}
