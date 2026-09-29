use jsengine::{Addons, Engine, Failure, Value};
use pt::{Fault, bundle};

struct Bare;

impl Addons for Bare {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        panic!("boundary component should not call native addons")
    }
}

#[test]
fn terminal() {
    let boundary = include_str!("../../../packages/composition/src/boundary.js");
    let artifact = bundle(
        "main.js",
        |path| match path {
            "main.js" => Ok(r#"
import Boundary from "./boundary.js";
const node = (tag) => ({
    tag: tag, children: [], attributes: {}, text: "",
    appendChild(child) { this.children[this.children.length] = child; },
    removeChild(child) { this.children.splice(this.children.indexOf(child), 1); },
    setAttribute(name, value) { this.attributes[name] = value; },
    setText(value) { this.text = value; }
});
const scope = {
    boundary(setup, recover) {
        const control = {generation: 0, failed: false, reset: () => false, dispose() {}};
        try { setup((callback) => callback); }
        catch (error) {
            control.failed = true;
            recover(error, control.reset, {
                primary: error, secondary: undefined,
                phase: "content", generation: 0, owner: control
            });
        }
        return control;
    }
};
const document = {createElement: node};
const host = node("section");
const reports = [];
const region = Boundary({
    document: document, host: host, scope: scope,
    content: () => { throw new Error("content"); },
    failed: () => { throw new Error("fallback"); },
    report: (failure) => { reports[reports.length] = failure; }
});
export const result = region.failed
    && host.children.length === 1
    && host.children[0].text === "Content failed"
    && host.children[0].attributes.role === "alert"
    && reports.length === 1
    && reports[0].primary.message === "content"
    && reports[0].secondary.message === "fallback"
    && reports[0].phase === "failed";
"#
            .into()),
            "boundary.js" => Ok(boundary.into()),
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
