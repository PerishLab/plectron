use super::paragraph::{Lines, Paragraph};
use super::routing::{Change, Plan, Rule};
use crate::Fault;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Weak},
};

pub(in crate::layout) struct Access {
    pub reuse: bool,
    pub remember: bool,
}

#[derive(Clone)]
pub(in crate::layout) enum Propagation {
    Unchanged { baseline: Arc<Lines> },
    Unknown,
}

#[derive(Clone)]
pub(in crate::layout) struct Output {
    pub lines: Arc<Lines>,
    pub propagation: Propagation,
    width: Option<usize>,
}

impl Output {
    pub fn build(paragraph: Paragraph, request: (Option<usize>, bool)) -> Result<Self, Fault> {
        let (width, editing) = request;
        paragraph.layout(width, editing).map(|lines| Self {
            lines: Arc::new(lines),
            propagation: Propagation::Unknown,
            width,
        })
    }
}

#[derive(Clone)]
pub(in crate::layout) struct Node {
    rule: Option<Rule>,
    previous: Rc<RefCell<Reference>>,
}

#[derive(Default)]
struct Reference {
    width: Option<usize>,
    lines: Weak<Lines>,
}

impl Node {
    pub fn new(paragraph: &Paragraph) -> Self {
        Self {
            rule: paragraph.rule(),
            previous: Rc::default(),
        }
    }
    pub fn run(
        &self,
        paragraph: Paragraph,
        request: (Option<usize>, bool),
        access: Access,
    ) -> Result<Output, Fault> {
        use pipeline::Graph;
        pipeline::step(self).run((paragraph, request, access))
    }
    fn execute(
        &self,
        paragraph: Paragraph,
        request: (Option<usize>, bool),
        access: Access,
    ) -> Result<Output, Fault> {
        let (width, editing) = request;
        if access.reuse
            && !editing
            && let Some(rule) = self.rule
        {
            if !rule.accepts(width) {
                paragraph.routed(false);
                return Output::build(paragraph, request);
            }
            let previous = self.previous.borrow();
            if let Some(lines) = previous.lines.upgrade() {
                let plan = rule.select(Change::Width {
                    before: previous.width,
                    after: width,
                });
                paragraph.routed(plan == Plan::Retain);
                if plan == Plan::Retain {
                    return Ok(Output {
                        lines: lines.clone(),
                        propagation: Propagation::Unchanged { baseline: lines },
                        width,
                    });
                }
            }
        }
        let output = Output::build(paragraph, request)?;
        if access.remember && !editing && self.rule.is_some_and(|rule| rule.accepts(width)) {
            *self.previous.borrow_mut() = Reference {
                width: output.width,
                lines: Arc::downgrade(&output.lines),
            };
        }
        Ok(output)
    }
    pub fn cost(&self) -> usize {
        std::mem::size_of::<RefCell<Reference>>() + 2 * std::mem::size_of::<usize>()
    }
}

impl pipeline::Node for &Node {
    type Input = (Paragraph, (Option<usize>, bool), Access);
    type Output = Output;
    type Error = Fault;
    const NAME: &'static str = "text.lines";
    fn run(&mut self, (paragraph, request, access): Self::Input) -> Result<Output, Fault> {
        self.execute(paragraph, request, access)
    }
}
