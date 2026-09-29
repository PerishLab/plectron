use crate::Fault;
use domengine::{Layout, Node};
use graphics::shared::Frame;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    observation: Observation<Option<Node>>,
}

impl Hit {
    pub fn target(&self) -> Option<usize> {
        self.observation.value.map(Node::id)
    }
    pub fn frame(&self) -> u64 {
        self.observation.basis.frame
    }
    pub fn scene(&self) -> u64 {
        self.observation.basis.scene
    }
    pub(crate) fn node(&self) -> Option<Node> {
        self.observation.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Basis {
    pub frame: u64,
    pub scene: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Observation<T> {
    pub basis: Basis,
    pub value: T,
}

#[derive(Default)]
pub(crate) struct Presentation {
    pub pending: Option<Frame>,
    pub issued: Option<View>,
    pub visible: Option<View>,
    pub fault: Option<String>,
}

pub(crate) struct View {
    pub frame: Frame,
    pub layout: Arc<Layout>,
}
impl View {
    pub fn hit(&self, x: usize, y: usize) -> Option<Node> {
        self.layout.pick(&self.frame, x, y)
    }
    pub(crate) fn observe<T>(&self, value: T) -> Observation<T> {
        Observation {
            basis: Basis {
                frame: self.frame.id(),
                scene: self.frame.scene().id(),
            },
            value,
        }
    }
    pub(crate) fn scroller(&self, x: usize, y: usize) -> Observation<Option<u64>> {
        let root = self.frame.mapping(0).expect("root space");
        self.observe(
            root.clip
                .contains(x, y)
                .then(|| self.layout.scroller(&self.frame, x, y))
                .flatten(),
        )
    }
}

impl Presentation {
    pub fn frame(&mut self, layout: &Option<Arc<Layout>>) -> Option<Frame> {
        if self.issued.is_some() || self.fault.is_some() {
            return None;
        }
        let frame = self.pending.take()?;
        self.issued = Some(View {
            frame: frame.clone(),
            layout: layout.as_ref()?.clone(),
        });
        Some(frame)
    }
    pub fn complete(&mut self, id: u64, result: Result<(), Fault>) -> Result<bool, Fault> {
        if self.issued.as_ref().map(|view| view.frame.id()) != Some(id) {
            return Err(Fault("frame acknowledgement is stale or unknown".into()));
        }
        let first = self.visible.is_none();
        let issued = self.issued.take();
        match result {
            Ok(()) => self.visible = issued,
            Err(fault) => {
                self.fault = Some(fault.to_string());
                self.pending = None;
                return Ok(false);
            }
        }
        Ok(first)
    }
}

impl super::Display {
    pub fn hit(&self, x: usize, y: usize) -> Hit {
        self.presentation.visible.as_ref().map_or_else(
            || Hit {
                observation: Observation {
                    basis: Basis { frame: 0, scene: 0 },
                    value: None,
                },
            },
            |view| Hit {
                observation: view.observe(view.hit(x, y)),
            },
        )
    }
}
