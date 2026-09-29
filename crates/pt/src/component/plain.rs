use super::{Parser, fields, intrinsic, literal};
use crate::{Artifact, Fault};

pub fn compile(source: &str) -> Result<Artifact, Fault> {
    let start = source
        .find("<template ")
        .ok_or_else(|| Fault(".pt requires a template".into()))?;
    let (prefix, template) = source.split_at(start);
    if !prefix.trim().is_empty() {
        return Err(Fault(".pt accepts only a template".into()));
    }
    let (header, rest) = template
        .split_once('>')
        .ok_or_else(|| Fault("template header is not closed".into()))?;
    let attrs = fields(header.trim_start_matches("<template"))?;
    let tag = attrs
        .iter()
        .find(|(name, _)| name == "tag")
        .map(|(_, value)| value)
        .ok_or_else(|| Fault("template requires tag".into()))?;
    if !intrinsic(tag) {
        return Err(Fault("template requires an intrinsic tag".into()));
    }
    let mut root = String::new();
    let mut listeners = String::new();
    for (name, value) in &attrs {
        if name == "tag" {
            continue;
        }
        if let Some(event) = name.strip_prefix('@') {
            listeners.push_str(&format!(
                "host.addEventListener({event:?}, handlers.{value}, {{scope: scope}});\n"
            ));
        } else if literal(name) {
            root.push_str(&format!("host.setAttribute({name:?}, {value:?});\n"));
        } else {
            return Err(Fault(format!("unsupported template attribute: {name}")));
        }
    }
    let (markup, tail) = rest
        .split_once("</template>")
        .ok_or_else(|| Fault("template is not closed".into()))?;
    if !tail.trim().is_empty() {
        return Err(Fault("unsupported content after template".into()));
    }
    let mut parser = Parser::new(markup);
    parser.stack.push((tag.clone(), "host".into()));
    parser.parse()?;
    if tag == "img" && !parser.code.is_empty() {
        return Err(Fault("img cannot contain children".into()));
    }
    listeners.push_str(&parser.listeners);
    let (method, result) = if listeners.is_empty() {
        (String::new(), "{host: host, refs: refs}")
    } else {
        (
            format!("const listen = (handlers, scope) => {{\n{listeners}}};\n"),
            "{host: host, refs: refs, listen: listen}",
        )
    };
    Ok(Artifact {
        javascript: format!(
            "export default function Template(document) {{\nconst host = document.createElement({tag:?});\nconst refs = {{}};\n{root}{}{method}return {result};\n}};\n",
            parser.code,
        ),
        entry: None,
    })
}
