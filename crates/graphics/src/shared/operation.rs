use crate::shared::{
    Backend, Extent, Fault, Frame, Item, Output, Pass, Preparation, Scene, State, Visual,
};
use pipeline::{Graph, Node, step};

pub struct Publication;
pub struct Source {
    pub extent: Extent,
    pub revision: u64,
    pub items: Vec<Item>,
}
impl Node for Publication {
    type Input = Source;
    type Output = Scene;
    type Error = Fault;
    const NAME: &'static str = "render.scene";
    fn run(&mut self, source: Source) -> Result<Scene, Fault> {
        Scene::publish(source.extent, source.revision, source.items)
    }
}

pub struct Mapping;
pub struct Revision<'a> {
    pub previous: &'a Scene,
    pub limit: usize,
}
impl Node for Revision<'_> {
    type Input = Source;
    type Output = Scene;
    type Error = Fault;
    const NAME: &'static str = "render.scene";
    fn run(&mut self, source: Source) -> Result<Scene, Fault> {
        self.previous.revise(source, self.limit)
    }
}

pub struct Remapping<'a> {
    pub previous: &'a Frame,
}
impl Node for Remapping<'_> {
    type Input = (Scene, Visual);
    type Output = Frame;
    type Error = Fault;
    const NAME: &'static str = "render.mapping";
    fn run(&mut self, (scene, visual): Self::Input) -> Result<Frame, Fault> {
        self.previous.revise(scene, visual)
    }
}
impl Node for Mapping {
    type Input = (Scene, Visual);
    type Output = Frame;
    type Error = Fault;
    const NAME: &'static str = "render.mapping";
    fn run(&mut self, (scene, visual): Self::Input) -> Result<Frame, Fault> {
        Frame::build(scene, visual)
    }
}

pub struct Prepare<'a> {
    pub engine: &'a mut Preparation,
}
impl<'a> Node for Prepare<'a> {
    type Input = (&'a Frame, Output);
    type Output = Pass;
    type Error = Fault;
    const NAME: &'static str = "render.preparation";
    fn run(&mut self, (frame, output): Self::Input) -> Result<Pass, Fault> {
        self.engine.build(frame, output)
    }
}

pub struct Submit<'a, B> {
    pub backend: &'a mut B,
}
impl<B: Backend> Node for Submit<'_, B> {
    type Input = Pass;
    type Output = B::Work;
    type Error = Fault;
    const NAME: &'static str = "render.submission";
    fn run(&mut self, pass: Pass) -> Result<B::Work, Fault> {
        self.backend.submit(&pass)
    }
}

pub struct Completion<'a, B> {
    pub backend: &'a mut B,
}
impl<'a, B: Backend> Node for Completion<'a, B> {
    type Input = &'a mut B::Work;
    type Output = State;
    type Error = Fault;
    const NAME: &'static str = "render.completion";
    fn run(&mut self, work: Self::Input) -> Result<State, Fault> {
        self.backend.poll(work)
    }
}

pub fn submission<'a, B: Backend>(
    engine: &'a mut Preparation,
    backend: &'a mut B,
) -> impl Graph<Input = (&'a Frame, Output), Output = B::Work, Error = Fault> {
    step(Prepare { engine }).then(step(Submit { backend }))
}
