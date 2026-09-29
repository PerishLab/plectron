use crate::Fault;
use crate::layout::{Constraints, Input, Layout, Painter, Pipeline, Shaper};
use pipeline::{Graph, Node, step};

pub struct Measurement<'a, S> {
    pub engine: &'a mut Pipeline<S>,
}
impl<'a, S: Shaper> Node for Measurement<'a, S> {
    type Input = (&'a Input, Constraints);
    type Output = Layout;
    type Error = Fault;
    const NAME: &'static str = "dom.layout";
    fn run(&mut self, (input, constraints): Self::Input) -> Result<Layout, Fault> {
        self.engine.build(input, constraints)
    }
}

pub struct Recording<'a> {
    pub painter: &'a mut Painter,
}
pub struct Painted {
    pub layout: Layout,
    pub items: Vec<graphics::shared::Item>,
}
impl Node for Recording<'_> {
    type Input = Layout;
    type Output = Painted;
    type Error = Fault;
    const NAME: &'static str = "dom.paint";
    fn run(&mut self, layout: Layout) -> Result<Painted, Fault> {
        let items = self.painter.build(&layout)?;
        Ok(Painted { layout, items })
    }
}

pub fn recording<'a, S: Shaper>(
    engine: &'a mut Pipeline<S>,
    painter: &'a mut Painter,
) -> impl Graph<Input = (&'a Input, Constraints), Output = Painted, Error = Fault> {
    step(Measurement { engine }).then(step(Recording { painter }))
}
