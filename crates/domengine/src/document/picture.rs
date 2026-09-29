use super::{Document, Kind, Node};
use crate::{Fault, change::Fact};
use graphics::shared::Raster;

pub(super) struct Binding {
    pub source: String,
    pub raster: Raster,
}

impl Document {
    pub fn image(&mut self, node: Node, source: &str, raster: Raster) -> Result<bool, Fault> {
        let entry = self.entry(node)?;
        if !matches!(&entry.kind, Kind::Element(tag) if tag == "img") {
            return Err(Fault("image binding requires img".into()));
        }
        if entry.attributes.get("src").map(String::as_str) != Some(source) {
            return Ok(false);
        }
        if self
            .pictures
            .get(&node)
            .is_some_and(|previous| previous.source == source && previous.raster == raster)
        {
            return Ok(true);
        }
        let connected = self.connected(node)?;
        self.pictures.insert(
            node,
            Binding {
                source: source.into(),
                raster,
            },
        );
        if connected {
            self.record(Fact::Image { node });
        }
        Ok(true)
    }
    pub fn picture(&self, node: Node) -> Result<Option<&Raster>, Fault> {
        let entry = self.entry(node)?;
        Ok(self
            .pictures
            .get(&node)
            .filter(|binding| entry.attributes.get("src") == Some(&binding.source))
            .map(|binding| &binding.raster))
    }
}
