use domengine::{Constraints, Fault, Input, Layout, Painter, Pipeline, Shaper, operation::Painted};
use graphics::shared::{
    Scene,
    operation::{Publication, Source},
};
use pipeline::{Graph, Node, step};

pub(super) struct Publish<'a> {
    timing: &'a mut u64,
    history: (Option<&'a Scene>, usize),
}
impl Node for Publish<'_> {
    type Input = Painted;
    type Output = (Layout, Scene);
    type Error = Fault;
    const NAME: &'static str = "web.publication";
    fn run(&mut self, painted: Painted) -> Result<Self::Output, Fault> {
        let source = Source {
            extent: painted.layout.extent(),
            revision: painted.layout.revision(),
            items: painted.items,
        };
        let start = std::time::Instant::now();
        let scene = if let Some(previous) = self.history.0 {
            step(graphics::shared::operation::Revision {
                previous,
                limit: self.history.1,
            })
            .run(source)
        } else {
            step(Publication).run(source)
        };
        *self.timing = self.timing.saturating_add(super::timing::elapsed(start));
        Ok((
            painted.layout,
            scene.map_err(|fault| Fault(fault.to_string()))?,
        ))
    }
}

pub fn publication<'a, S: Shaper>(
    engine: &'a mut Pipeline<S>,
    painter: &'a mut Painter,
    timing: &'a mut u64,
    history: (Option<&'a Scene>, usize),
) -> impl Graph<Input = (&'a Input, Constraints), Output = (Layout, Scene), Error = Fault> {
    domengine::operation::recording(engine, painter).then(step(Publish { timing, history }))
}
