use crate::{
    Node,
    layout::{
        delta::Transition,
        input::{Editor, Element, Inline, Origin, Record},
    },
    style::Style,
};
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::layout) struct Material(Arc<Record>);

impl std::fmt::Debug for Material {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Material")
            .field("node", &self.node())
            .finish()
    }
}

#[derive(Clone)]
pub(in crate::layout) struct History(Origin);

#[derive(PartialEq)]
struct Content<'a> {
    node: Node,
    target: Node,
    text: &'a Option<Arc<str>>,
    picture: &'a Option<graphics::shared::Raster>,
    editor: &'a Option<Editor>,
}

impl<'a> Content<'a> {
    fn new(record: &'a Element) -> Self {
        Self {
            node: record.node,
            target: record.target,
            text: &record.text,
            picture: &record.picture,
            editor: &record.editor,
        }
    }
}

impl Material {
    pub(in crate::layout) fn new(record: Arc<Record>) -> Self {
        Self(record)
    }
    pub fn node(&self) -> Node {
        self.0.node
    }
    pub fn target(&self) -> Node {
        self.0.target
    }
    pub fn style(&self) -> &Style {
        &self.0.style
    }
    pub fn text(&self) -> Option<&Arc<str>> {
        self.0.text.as_ref()
    }
    pub fn picture(&self) -> Option<&graphics::shared::Raster> {
        self.0.picture.as_ref()
    }
    pub fn editor(&self) -> Option<&Editor> {
        self.0.editor.as_ref()
    }
    pub fn inline(&self) -> Option<&Arc<Inline>> {
        self.0.inline.as_ref()
    }
    pub fn isolated(&self) -> bool {
        self.0.key.isolated
    }
    pub fn independent(&self) -> bool {
        self.0.key.style.independent()
    }
    pub(in crate::layout) fn fingerprint(&self) -> u64 {
        self.0.key.fingerprint()
    }
    pub(in crate::layout) fn estimated(&self) -> bool {
        self.0.key.style.intrinsic.is_some()
    }
    pub fn matches(&self, other: &Self) -> bool {
        self.0.matches(&other.0)
    }
    pub fn history(&self) -> History {
        History(self.0.origin())
    }
    pub fn owned(&self, history: &History) -> bool {
        history.0.matches(&self.0)
    }
    pub fn transition(&self, history: &History) -> Option<Transition> {
        let current = self.0.capture()?;
        let previous = history.0.resolve()?;
        if (current.style.translation == previous.style.translation
            && current.style.opacity == previous.style.opacity)
            || !Arc::ptr_eq(&current.key, &previous.key)
        {
            return None;
        }
        if Content::new(&current) != Content::new(&previous) {
            return None;
        }
        if !shared(&current.children, &previous.children) || !shared(&current.flow, &previous.flow)
        {
            return None;
        }
        let mut style = (*previous.style).clone();
        style.translation = current.style.translation;
        style.opacity = current.style.opacity;
        if style != *current.style {
            return None;
        }
        let translation = [
            current.style.translation[0].checked_sub(previous.style.translation[0])?,
            current.style.translation[1].checked_sub(previous.style.translation[1])?,
        ];
        Some(
            match (translation, current.style.opacity == previous.style.opacity) {
                ([0, 0], _) => Transition::Opacity(current.style.opacity),
                (_, true) => Transition::Translate(translation),
                (_, false) => Transition::Composite {
                    translation,
                    opacity: current.style.opacity,
                },
            },
        )
    }
}

fn shared<T>(current: &[Arc<T>], previous: &[Arc<T>]) -> bool {
    current.len() == previous.len()
        && current
            .iter()
            .zip(previous)
            .all(|(current, previous)| Arc::ptr_eq(current, previous))
}
