use super::{Caret, Editor, Element, Input, Key, Tree, damage, key::Description};
use crate::{
    Document, Fault, Kind, Node, Sheet,
    change::Version,
    style::{Inherited, Resolver, Style},
};
use std::sync::Arc;

pub(super) struct Context<'a> {
    pub style: Arc<Style>,
    pub inherited: Inherited,
    pub previous: Option<&'a Arc<Element>>,
    pub identity: Option<&'a Arc<Element>>,
}
impl Context<'_> {
    fn text(&self, text: &str) -> Arc<str> {
        self.previous
            .and_then(|old| old.text.as_ref())
            .filter(|old| old.as_ref() == text)
            .cloned()
            .unwrap_or_else(|| Arc::from(text))
    }
}
impl Element {
    pub(super) fn capture(
        node: Node,
        document: &Document,
        resolve: &mut impl FnMut(Node, Inherited) -> Result<Arc<Style>, Fault>,
        inherited: Inherited,
    ) -> Result<Arc<Self>, Fault> {
        let style = resolve(node, inherited.clone())?;
        let children = document
            .entry(node)?
            .children
            .iter()
            .map(|child| Self::capture(*child, document, resolve, Inherited::from(style.as_ref())))
            .collect::<Result<Vec<_>, _>>()?;
        Self::finish(
            document,
            node,
            Context {
                style,
                inherited,
                previous: None,
                identity: None,
            },
            children,
        )
        .map(Arc::new)
    }
    pub(super) fn finish(
        document: &Document,
        node: Node,
        context: Context<'_>,
        mut children: Vec<Arc<Self>>,
    ) -> Result<Self, Fault> {
        let entry = document.entry(node)?;
        let text = match &entry.kind {
            Kind::Text(text) => Some(context.text(text)),
            _ => None,
        };
        let Context {
            style,
            inherited,
            previous,
            identity,
        } = context;
        let target = if text.is_some() {
            entry.parent.unwrap_or(node)
        } else {
            node
        };
        let count = 1 + children.iter().map(|child| child.count).sum::<usize>();
        let legacy = matches!(&entry.kind, Kind::Element(tag) if tag == "input");
        let generated = document.cursor(node).is_some();
        let buffer = (legacy || generated)
            .then(|| document.buffer(node))
            .transpose()?;
        if legacy && let Some(buffer) = buffer {
            let style = Arc::new(Style {
                color: style.color,
                typography: style.typography.clone(),
                ..Style::default()
            });
            let projection = buffer.projection();
            let text = Some(Arc::from(projection.text));
            let identity = identity.and_then(|old| {
                old.children
                    .iter()
                    .find(|child| child.node == node && child.editor.is_some())
            });
            let key = Key::new(&style, (&text, &[]), (true, None), &[]);
            children.push(Arc::new(Self {
                id: identity.map_or_else(super::Id::fresh, |child| child.id),
                version: Self::version(&key, identity),
                key,
                node,
                target: node,
                inherited: Inherited::from(style.as_ref()),
                style,
                text,
                picture: None,
                editor: Some(Editor {
                    offsets: projection.offsets,
                    active: document.focused() == Some(node),
                    caret: None,
                }),
                children: Vec::new(),
                flow: Vec::new(),
                inline: None,
                count: 0,
            }));
        }
        let parts = generated
            .then(|| document.parts.get(&node).copied())
            .flatten();
        if let (Some(parts), Some(buffer)) = (parts, buffer) {
            let caret = children
                .iter()
                .find(|child| child.node == parts.caret)
                .map(|child| {
                    let style = child.style.as_ref();
                    Caret {
                        node: parts.caret,
                        color: style.background.unwrap_or(style.color),
                        opacity: if style.display == crate::style::Display::None {
                            0
                        } else {
                            style.opacity
                        },
                        width: style.width.unwrap_or(1),
                        height: style.height,
                        translation: style.translation,
                    }
                });
            if let Some(child) = children.iter_mut().find(|child| child.node == parts.text) {
                let child = Arc::make_mut(child);
                child.editor = Some(Editor {
                    offsets: buffer.projection().offsets,
                    active: document.focused() == Some(node),
                    caret,
                });
                child.key = Key::new(&child.style, (&child.text, &[]), (true, None), &[]);
            }
        }
        let picture = matches!(&entry.kind, Kind::Element(tag) if tag == "img")
            .then(|| document.picture(node))
            .transpose()?
            .flatten()
            .cloned();
        if matches!(&entry.kind, Kind::Element(tag) if tag == "img") && picture.is_none() {
            return Err(Fault("image source has no resolved raster".into()));
        }
        let flow = if let Some(parts) = parts {
            let visible = children
                .iter()
                .filter(|child| child.node != parts.caret)
                .cloned()
                .collect::<Vec<_>>();
            super::flow::build(&visible, &style, identity.map(|old| old.flow.as_slice()))?
        } else {
            super::flow::build(&children, &style, identity.map(|old| old.flow.as_slice()))?
        };
        let key = Description::new(
            &style,
            (&text, &[]),
            (
                false,
                picture.as_ref().map(graphics::shared::Raster::extent),
            ),
            &flow,
        )
        .resolve(previous.map(|old| &old.key));
        Ok(Self {
            id: identity.map_or_else(super::Id::fresh, |old| old.id),
            version: Self::version(&key, identity),
            key,
            flow,
            inline: None,
            picture,
            node,
            target,
            style,
            text,
            editor: None,
            children,
            inherited,
            count,
        })
    }
}

impl Input {
    pub fn tree(&self) -> &Arc<Tree> {
        &self.tree
    }
    pub fn version(&self) -> Version {
        Version {
            root: self.tree.element().node,
            revision: self.revision,
        }
    }
    pub fn capture(document: &Document, sheet: &Sheet) -> Result<Self, Fault> {
        let revision = document.revision();
        let root = Element::capture(
            document.root(),
            document,
            &mut |node, color| sheet.compute(document, node, color).map(Arc::new),
            Inherited::default(),
        )?;
        Ok(Self {
            revision,
            tree: Tree::build(&root, revision, damage::Draft::full(), None).0,
        })
    }
    pub fn resolve(document: &Document, resolver: &mut Resolver) -> Result<Self, Fault> {
        resolver.maintain();
        let root = Element::capture(
            document.root(),
            document,
            &mut |node, inherited| {
                resolver.resolve(crate::style::Subject::capture(document, node)?, inherited)
            },
            Inherited::default(),
        );
        resolver.publish();
        let revision = document.revision();
        let root = root?;
        Ok(Self {
            revision,
            tree: Tree::build(&root, revision, damage::Draft::full(), None).0,
        })
    }
}
