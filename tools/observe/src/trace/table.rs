use crate::Result;
use roxmltree::{Document, Node};
use std::collections::HashMap;

pub struct Table<'a> {
    document: Document<'a>,
}

impl<'a> Table<'a> {
    pub fn new(source: &'a str) -> Result<Self> {
        let document = Document::parse(source).map_err(|error| error.to_string())?;
        Ok(Self { document })
    }

    pub fn resolve<'d>(&'d self, mut node: Option<Node<'d, 'a>>) -> Option<Node<'d, 'a>> {
        let mut seen = Vec::new();
        while let Some(reference) = node.and_then(|value| value.attribute("ref")) {
            if seen.contains(&reference) {
                return None;
            }
            seen.push(reference);
            node = self
                .document
                .descendants()
                .find(|candidate| candidate.attribute("id") == Some(reference));
        }
        node
    }

    pub fn text<'d>(&'d self, node: Option<Node<'d, 'a>>) -> Option<&'d str> {
        self.resolve(node).and_then(|node| node.text())
    }

    pub fn child<'d>(&'d self, node: Option<Node<'d, 'a>>, name: &str) -> Option<Node<'d, 'a>> {
        self.resolve(node)?
            .children()
            .find(|child| child.has_tag_name(name))
    }

    pub fn frames<'d>(&'d self, node: Node<'d, 'a>) -> Vec<String> {
        let Some(node) = self.resolve(Some(node)) else {
            return Vec::new();
        };
        if node.has_tag_name("frame") {
            return vec![node.attribute("name").unwrap_or("?").into()];
        }
        node.children()
            .flat_map(|child| self.frames(child))
            .collect()
    }

    pub fn rows<'d>(&'d self) -> Vec<HashMap<String, Node<'d, 'a>>> {
        let mut rows = Vec::new();
        for node in self
            .document
            .descendants()
            .filter(|node| node.has_tag_name("node"))
        {
            let Some(schema) = node.children().find(|child| child.has_tag_name("schema")) else {
                continue;
            };
            let keys = schema
                .children()
                .filter(|child| child.has_tag_name("col"))
                .map(|column| {
                    column
                        .descendants()
                        .find(|child| child.has_tag_name("mnemonic"))
                        .and_then(|child| child.text())
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect::<Vec<_>>();
            for row in node.children().filter(|child| child.has_tag_name("row")) {
                rows.push(
                    keys.iter()
                        .cloned()
                        .zip(row.children().filter(Node::is_element))
                        .collect(),
                );
            }
        }
        rows
    }
}

pub fn parse(value: Option<&str>, label: &str) -> Result<i64> {
    value
        .ok_or_else(|| format!("{label} is absent"))?
        .parse()
        .map_err(|error| format!("{label}: {error}"))
}
