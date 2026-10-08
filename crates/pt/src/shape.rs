use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
pub struct Shape {
    pub host: String,
    pub refs: BTreeMap<String, String>,
    pub handlers: BTreeSet<String>,
    pub text: bool,
}

impl Shape {
    pub fn declaration(&self) -> String {
        let mut result = String::from(
            "export default function Template<D extends {createElement: (tag: string) => unknown;",
        );
        if self.text {
            result.push_str(" createTextNode: (text: string) => unknown;");
        }
        result
            .push_str("}>(document: D): {\n  host: ReturnType<D[\"createElement\"]>;\n  refs: {\n");
        for (name, tag) in &self.refs {
            let factory = if tag == "text" {
                "createTextNode"
            } else {
                "createElement"
            };
            result.push_str(&format!("    {name}: ReturnType<D[\"{factory}\"]>;\n"));
        }
        result.push_str("  };\n");
        if !self.handlers.is_empty() {
            result.push_str("  listen: (handlers: {\n");
            for handler in &self.handlers {
                result.push_str(&format!("    {handler}: (...values: any[]) => unknown;\n"));
            }
            result.push_str("  }, scope: unknown) => void;\n");
        }
        result.push_str("};\n");
        result
    }
}
