use super::{Editor, Element, Inline, Tree, sizing::Sizing};
use crate::{
    Node,
    style::{Display, Style},
};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

#[derive(Clone, Copy)]
pub(in crate::layout) struct Entry<'a> {
    pub(super) tree: &'a Tree,
    pub(super) slot: usize,
}

impl<'a> Entry<'a> {
    pub(in crate::layout) fn id(self) -> super::Id {
        self.tree.nodes()[self.slot].id
    }

    pub(in crate::layout) fn parent(self) -> Option<super::Id> {
        self.tree.nodes()[self.slot].parent
    }

    pub fn record(self) -> &'a Arc<Record> {
        &self.tree.records[self.slot]
    }

    pub fn children(self) -> impl Iterator<Item = Self> {
        self.tree.children(self.slot).map(move |slot| Self {
            tree: self.tree,
            slot,
        })
    }
}

impl std::ops::Deref for Entry<'_> {
    type Target = Record;

    fn deref(&self) -> &Self::Target {
        &self.tree.records[self.slot]
    }
}

pub(in crate::layout) struct Record {
    pub key: Arc<Key>,
    pub node: Node,
    pub target: Node,
    pub style: Arc<Style>,
    pub text: Option<Arc<str>>,
    pub picture: Option<graphics::shared::Raster>,
    pub editor: Option<Editor>,
    pub inline: Option<Arc<Inline>>,
    pub edited: bool,
    capture: Weak<Element>,
}

#[derive(PartialEq)]
struct Identity<'a> {
    key: &'a Local,
    node: Node,
    target: Node,
    style: &'a Arc<Style>,
    text: &'a Option<Arc<str>>,
    picture: &'a Option<graphics::shared::Raster>,
    editor: &'a Option<Editor>,
    inline: &'a Option<Arc<Inline>>,
    edited: bool,
}

#[derive(Clone)]
pub(in crate::layout) struct Origin(Weak<Element>);

impl Origin {
    pub(in crate::layout) fn matches(&self, record: &Record) -> bool {
        self.0.as_ptr() == record.capture.as_ptr()
    }

    pub(in crate::layout) fn resolve(&self) -> Option<Arc<Element>> {
        self.0.upgrade()
    }
}

impl Record {
    pub(super) fn new(capture: &Arc<Element>) -> Arc<Self> {
        Arc::new(Self {
            key: capture.key.clone(),
            node: capture.node,
            target: capture.target,
            style: capture.style.clone(),
            text: capture.text.clone(),
            picture: capture.picture.clone(),
            editor: capture.editor.clone(),
            inline: capture.inline.clone(),
            edited: capture.children.iter().any(|child| child.key.editing),
            capture: Arc::downgrade(capture),
        })
    }

    pub(in crate::layout) fn capture(&self) -> Option<Arc<Element>> {
        self.capture.upgrade()
    }

    pub(in crate::layout) fn alive(&self) -> bool {
        self.capture.strong_count() > 0
    }

    pub(in crate::layout) fn matches(&self, other: &Self) -> bool {
        self.capture.as_ptr() == other.capture.as_ptr()
    }

    pub(in crate::layout) fn same(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }

    pub(in crate::layout) fn origin(&self) -> Origin {
        Origin(self.capture.clone())
    }

    fn identity(&self) -> Identity<'_> {
        Identity {
            key: self.key.local(),
            node: self.node,
            target: self.target,
            style: &self.style,
            text: &self.text,
            picture: &self.picture,
            editor: &self.editor,
            inline: &self.inline,
            edited: self.edited,
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(in crate::layout) struct Local {
    pub style: Sizing,
    pub text: Option<Arc<str>>,
    pub editing: bool,
    pub image: Option<crate::Extent>,
    pub fonts: Vec<super::Section>,
}

#[derive(PartialEq, Eq)]
pub(in crate::layout) struct Key {
    local: Local,
    children: Vec<Arc<Key>>,
    pub weight: usize,
    pub isolated: bool,
    fingerprint: u64,
}

impl std::ops::Deref for Key {
    type Target = Local;

    fn deref(&self) -> &Self::Target {
        &self.local
    }
}

impl Key {
    pub(in crate::layout) fn local(&self) -> &Local {
        &self.local
    }

    pub(in crate::layout) const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn new(
        style: &Style,
        text: (&Option<Arc<str>>, &[super::Section]),
        content: (bool, Option<crate::Extent>),
        elements: &[Arc<Element>],
    ) -> Arc<Self> {
        Description::new(style, text, content, elements).resolve(None)
    }
}

pub(super) struct Description<'a> {
    style: Sizing,
    text: &'a Option<Arc<str>>,
    fonts: &'a [super::Section],
    content: (bool, Option<crate::Extent>),
    elements: &'a [Arc<Element>],
}

#[derive(PartialEq)]
struct Content<'a> {
    style: &'a Sizing,
    text: &'a Option<Arc<str>>,
    fonts: &'a [super::Section],
    embedded: (bool, Option<crate::Extent>),
}
impl<'a> Description<'a> {
    pub fn new(
        style: &Style,
        text: (&'a Option<Arc<str>>, &'a [super::Section]),
        content: (bool, Option<crate::Extent>),
        elements: &'a [Arc<Element>],
    ) -> Self {
        let (text, fonts) = text;
        let mut style = Sizing::from(style);
        if !content.0
            && style.typography.whitespace.collapse()
            && text.as_ref().is_some_and(|text| {
                text.chars()
                    .all(|character| matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{000c}'))
            })
        {
            style.display = Display::None;
        }
        Self {
            style,
            text,
            fonts,
            content,
            elements,
        }
    }
    fn children(&self) -> impl Iterator<Item = &Arc<Key>> {
        self.elements
            .iter()
            .filter(|child| child.key.style.display != Display::None)
            .map(|child| &child.key)
    }
    pub fn resolve(self, previous: Option<&Arc<Key>>) -> Arc<Key> {
        if let Some(previous) = previous.filter(|previous| {
            Content {
                style: &previous.style,
                text: &previous.text,
                fonts: previous.fonts.as_slice(),
                embedded: (previous.editing, previous.image),
            } == Content {
                style: &self.style,
                text: self.text,
                fonts: self.fonts,
                embedded: self.content,
            } && previous.children.iter().eq(self.children())
        }) {
            return previous.clone();
        }
        let children: Vec<_> = self.children().cloned().collect();
        let weight = std::mem::size_of::<Key>()
            + self.style.typography.font.retained()
            + self
                .fonts
                .iter()
                .map(|section| std::mem::size_of::<super::Section>() + section.font.retained())
                .sum::<usize>()
            + self.text.as_ref().map_or(0, |text| text.len())
            + children.iter().map(|child| child.weight).sum::<usize>();
        let local = Local {
            style: self.style,
            text: self.text.clone(),
            editing: self.content.0,
            image: self.content.1,
            fonts: self.fonts.to_vec(),
        };
        let mut digest = ahash::AHasher::default();
        (&local, &children).hash(&mut digest);
        Arc::new(Key {
            isolated: local.style.independent() || children.iter().any(|child| child.isolated),
            local,
            children,
            weight,
            fingerprint: digest.finish(),
        })
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}
