use super::Pictures;
use crate::{Fault, error};
use domengine::{
    Document, Kind, Node,
    change::{Attribute, Fact},
};
use std::collections::BTreeSet;
impl Pictures {
    pub(super) fn discover(&mut self, document: &Document) -> Result<(), Fault> {
        let changes = self.version.and_then(|version| document.changes(version));
        let mut roots = BTreeSet::new();
        let mut singles = BTreeSet::new();
        if let Some(changes) = changes {
            self.report.changes += changes.len() as u64;
            (roots, singles) = affected(changes);
        } else {
            self.report.scans += 1;
            self.pending.clear();
            roots.insert(document.root());
        }
        let mut visited = BTreeSet::new();
        for root in roots {
            if document.connected(root).map_err(error)? {
                self.subtree(document, root, &mut visited)?;
            }
        }
        for node in singles {
            if document.connected(node).map_err(error)? && visited.insert(node) {
                self.visit(document, node)?;
            }
        }
        self.version = Some(document.version());
        Ok(())
    }
    fn subtree(
        &mut self,
        document: &Document,
        node: Node,
        visited: &mut BTreeSet<Node>,
    ) -> Result<(), Fault> {
        if !visited.insert(node) {
            return Ok(());
        }
        self.visit(document, node)?;
        for child in &document.entry(node).map_err(error)?.children {
            self.subtree(document, *child, visited)?;
        }
        Ok(())
    }
    fn visit(&mut self, document: &Document, node: Node) -> Result<(), Fault> {
        self.report.visited += 1;
        if matches!(&document.entry(node).map_err(error)?.kind, Kind::Element(tag) if tag == "img")
            && document.picture(node).map_err(error)?.is_none()
        {
            self.pending.insert(node);
        }
        Ok(())
    }
}

fn affected(changes: Vec<domengine::change::Change>) -> (BTreeSet<Node>, BTreeSet<Node>) {
    let mut roots = BTreeSet::new();
    let mut singles = BTreeSet::new();
    for change in changes {
        match change.fact {
            Fact::Attribute {
                node,
                name: Attribute::Src,
            } => {
                singles.insert(node);
            }
            Fact::Attached {
                node, to: Some(_), ..
            } => {
                roots.insert(node);
            }
            _ => {}
        }
    }
    (roots, singles)
}
