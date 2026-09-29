use crate::Fault;
use grammar::Grammar;
use shared::Resource;
use std::sync::OnceLock;

const JS: Resource = Resource::new(include_bytes!("../../resources/js.grammar"));

pub(super) fn js() -> Result<&'static Grammar, Fault> {
    static CACHE: OnceLock<Result<Grammar, Fault>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let source = JS.text().map_err(|error| Fault(error.to_string()))?;
            Grammar::compile(source).map_err(|error| Fault(error.to_string()))
        })
        .as_ref()
        .map_err(Clone::clone)
}
