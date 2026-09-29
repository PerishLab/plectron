use super::{Edge, Element, Entity, Id, Key, Record, Relation, Role};
use crate::{
    Fault, Node,
    style::{Display, Inherited, Style},
};
use std::{ops::Range, sync::Arc};

pub(super) struct Topology {
    pub nodes: Arc<[Entity]>,
    pub edges: Arc<[Edge]>,
    pub flows: Arc<[usize]>,
    pub records: Arc<[Arc<Record>]>,
}

pub(super) fn topology(root: &Arc<Element>) -> Topology {
    let mut graph = Graph::default();
    graph.append(root, None, false);
    Topology {
        nodes: graph.nodes.into(),
        edges: graph.edges.into(),
        flows: graph.flows.into(),
        records: graph.records.into(),
    }
}

#[derive(Default)]
struct Graph {
    nodes: Vec<Entity>,
    edges: Vec<Edge>,
    flows: Vec<usize>,
    records: Vec<Arc<Record>>,
}

impl Graph {
    fn append(&mut self, element: &Arc<Element>, parent: Option<(Id, Node)>, editor: bool) {
        let role = if element.inline.is_some() {
            Role::Anonymous
        } else if editor {
            Role::Editor
        } else {
            Role::Source
        };
        let start = self.flows.len();
        self.flows.resize(start + element.flow.len(), usize::MAX);
        self.nodes.push(Entity::new(
            (element.id, element.version),
            (
                parent.map(|value| value.0),
                if role == Role::Anonymous {
                    parent.map_or(element.node, |value| value.1)
                } else {
                    element.node
                }
                .source(),
            ),
            (role, element.key.style.independent()),
            start..start + element.flow.len(),
        ));
        self.records.push(Record::new(element));
        let mut previous = None;
        for child in &element.flow {
            self.edges.extend([
                Edge {
                    from: child.id,
                    to: element.id,
                    relation: Relation::Intrinsic,
                },
                Edge {
                    from: element.id,
                    to: child.id,
                    relation: Relation::Constraint,
                },
            ]);
            if let Some(from) = previous {
                self.edges.push(Edge {
                    from,
                    to: child.id,
                    relation: Relation::Flow,
                });
            }
            previous = Some(child.id);
        }
        for (offset, child) in element.flow.iter().enumerate() {
            self.flows[start + offset] = self.nodes.len();
            self.append(
                child,
                Some((element.id, element.node)),
                child.editor.is_some() && child.node == element.node,
            );
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::layout) struct Section {
    pub source: Range<usize>,
    pub font: textengine::Request,
}
#[derive(PartialEq, Eq)]
pub(in crate::layout) struct Leaf {
    pub source: Range<usize>,
    pub node: Node,
    pub target: Node,
    pub color: u32,
}
#[derive(PartialEq, Eq)]
pub(in crate::layout) struct Inline {
    pub leaves: Vec<Leaf>,
    pub atoms: Vec<Range<usize>>,
}
struct Builder {
    source: String,
    fonts: Vec<Section>,
    leaves: Vec<Leaf>,
    atoms: Vec<Range<usize>>,
    boxes: Vec<Arc<Element>>,
}
pub(super) fn build(
    children: &[Arc<Element>],
    style: &Style,
    previous: Option<&[Arc<Element>]>,
) -> Result<Vec<Arc<Element>>, Fault> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    for child in children {
        if child.style.display == Display::None {
            continue;
        }
        let inline = child.text.is_some() || child.style.display == Display::Inline;
        if inline && child.editor.is_none() && style.display != Display::Flex {
            pending.push(child.clone());
        } else {
            flush(&mut result, &mut pending, style, previous)?;
            result.push(child.clone());
        }
    }
    flush(&mut result, &mut pending, style, previous)?;
    Ok(result)
}
fn flush(
    result: &mut Vec<Arc<Element>>,
    pending: &mut Vec<Arc<Element>>,
    style: &Style,
    previous: Option<&[Arc<Element>]>,
) -> Result<(), Fault> {
    if pending.is_empty() {
        return Ok(());
    }
    if pending.len() == 1 && pending[0].text.is_some() {
        result.append(pending);
        return Ok(());
    }
    let mut builder = Builder {
        source: String::new(),
        fonts: Vec::new(),
        leaves: Vec::new(),
        atoms: Vec::new(),
        boxes: Vec::new(),
    };
    for element in pending.iter() {
        builder.append(element, style)?;
    }
    let first = &pending[0];
    let style = Style {
        color: style.color,
        typography: style.typography.clone(),
        ..Style::default()
    };
    let text = Some(Arc::from(builder.source.as_str()));
    let identity = previous.and_then(|old| {
        old.iter()
            .find(|element| element.inline.is_some() && element.node == first.node)
    });
    let key = Key::new(
        &style,
        (
            &text,
            &Section::normalize(&builder.source, &builder.fonts, &builder.atoms),
        ),
        (false, None),
        &builder.boxes,
    );
    result.push(Arc::new(Element {
        id: identity.map_or_else(super::Id::fresh, |element| element.id),
        version: Element::version(&key, identity),
        key,
        inherited: Inherited::from(&style),
        style: Arc::new(style),
        text,
        picture: None,
        node: first.node,
        target: first.target,
        editor: None,
        children: Vec::new(),
        flow: builder.boxes,
        inline: Some(Arc::new(Inline {
            leaves: builder.leaves,
            atoms: builder.atoms,
        })),
        count: 0,
    }));
    pending.clear();
    Ok(())
}
impl Builder {
    fn append(&mut self, element: &Arc<Element>, style: &Style) -> Result<(), Fault> {
        if element.style.display == Display::None {
            return Ok(());
        }
        if element.picture.is_some() {
            if element.style.display != Display::Inline {
                return Err(Fault("block inside inline is outside this profile".into()));
            }
            let start = self.source.len();
            if start + 3 > 65_536 {
                return Err(Fault("inline source budget exhausted".into()));
            }
            self.source.push('\u{fffc}');
            self.atoms.push(start..self.source.len());
            self.boxes.push(element.clone());
            return Ok(());
        }
        let mut expected = Style {
            color: element.style.color,
            typography: style.typography.clone(),
            display: element.style.display,
            ..Style::default()
        };
        expected.typography.font = element.style.typography.font.clone();
        if *element.style != expected {
            return Err(Fault("inline supports font and color changes; box effects and paragraph policy changes require a block".into()));
        }
        if let Some(text) = &element.text {
            let source = self.source.len()..self.source.len() + text.len();
            if source.end > 65_536 {
                return Err(Fault("inline source budget exhausted".into()));
            }
            self.source.push_str(text);
            if !source.is_empty() {
                self.fonts.push(Section {
                    source: source.clone(),
                    font: element.style.typography.font.clone(),
                });
                self.leaves.push(Leaf {
                    source,
                    node: element.node,
                    target: element.target,
                    color: element.style.color,
                });
            }
            return Ok(());
        }
        if element.style.display != Display::Inline {
            return Err(Fault("block inside inline is outside this profile".into()));
        }
        for child in &element.children {
            self.append(child, style)?;
        }
        Ok(())
    }
}
