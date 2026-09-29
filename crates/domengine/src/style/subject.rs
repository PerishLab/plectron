use crate::{Document, Fault, Kind, Node};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subject {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Option<String>,
    pub inline: Option<String>,
    pub direction: Option<String>,
}
impl Subject {
    pub fn capture(document: &Document, node: Node) -> Result<Self, Fault> {
        let entry = document.entry(node)?;
        Ok(Self {
            tag: match &entry.kind {
                Kind::Element(tag) => Some(tag.clone()),
                _ => None,
            },
            id: entry.attributes.get("id").cloned(),
            classes: entry.attributes.get("class").cloned(),
            inline: entry.attributes.get("style").cloned(),
            direction: entry.attributes.get("dir").cloned(),
        })
    }
    pub(super) fn matches(&self, selector: &str) -> bool {
        if self.tag.is_none() {
            return false;
        }
        if let Some(id) = selector.strip_prefix('#') {
            return self.id.as_deref() == Some(id);
        }
        if let Some(class) = selector.strip_prefix('.') {
            return self
                .classes
                .as_ref()
                .is_some_and(|value| value.split_whitespace().any(|item| item == class));
        }
        self.tag.as_deref() == Some(selector)
    }
    pub(super) fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + [
                &self.tag,
                &self.id,
                &self.classes,
                &self.inline,
                &self.direction,
            ]
            .iter()
            .map(|value| value.as_ref().map_or(0, String::len))
            .sum::<usize>()
    }
}
