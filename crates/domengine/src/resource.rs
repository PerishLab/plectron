use crate::Fault;
use grammar::Grammar;
use shared::Resource;
use std::sync::OnceLock;

const HTML: Resource = Resource::new(include_bytes!("../resources/html.grammar"));
const CSS: Resource = Resource::new(include_bytes!("../resources/css.grammar"));

pub(super) fn html() -> Result<&'static Grammar, Fault> {
    static CACHE: OnceLock<Result<Grammar, Fault>> = OnceLock::new();
    compile(HTML, &CACHE)
}

pub(super) fn css() -> Result<&'static Grammar, Fault> {
    static CACHE: OnceLock<Result<Grammar, Fault>> = OnceLock::new();
    compile(CSS, &CACHE)
}

fn compile(
    resource: Resource,
    cache: &'static OnceLock<Result<Grammar, Fault>>,
) -> Result<&'static Grammar, Fault> {
    cache
        .get_or_init(|| {
            let source = resource.text().map_err(|error| Fault(error.to_string()))?;
            Grammar::compile(source).map_err(|error| Fault(error.to_string()))
        })
        .as_ref()
        .map_err(Clone::clone)
}
