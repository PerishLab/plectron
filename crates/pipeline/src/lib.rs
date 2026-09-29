mod composition;
mod observation;
mod topology;
mod sealed {
    pub trait Sealed {}
}
pub use composition::{Pair, Step, Then};
pub use observation::{Observer, Silent, Span, Trace};
pub use topology::{Topology, Vertex};

pub trait Node {
    type Input;
    type Output;
    type Error;
    const NAME: &'static str;
    fn run(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

pub fn step<N: Node>(node: N) -> Step<N> {
    Step(node)
}

pub trait Graph: sealed::Sealed + Sized {
    type Input;
    type Output;
    type Error;
    const LEN: usize;
    fn evaluate(
        &mut self,
        input: Self::Input,
        observer: &mut impl Observer,
        offset: usize,
    ) -> Result<Self::Output, Self::Error>;
    fn describe(&self, topology: &mut Topology, incoming: &[usize]) -> Vec<usize>;

    fn run(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let mut scope = self.scope();
        let result = self.evaluate(input, &mut Silent, 0);
        scope.finish(result.is_ok());
        result
    }
    fn observe(
        &mut self,
        input: Self::Input,
        observer: &mut impl Observer,
    ) -> Result<Self::Output, Self::Error> {
        observer.reset();
        observer.begin(|| self.topology());
        let mut scope = self.scope();
        let result = self.evaluate(input, observer, 0);
        scope.finish(result.is_ok());
        result
    }
    fn scope(&self) -> observe::Guard {
        observe::graph(|| {
            self.topology()
                .vertices()
                .iter()
                .map(|vertex| observe::Vertex {
                    name: vertex.name.into(),
                    incoming: vertex.incoming.clone(),
                })
                .collect()
        })
    }
    fn topology(&self) -> Topology {
        let mut topology = Topology::default();
        self.describe(&mut topology, &[]);
        topology
    }
    fn then<B>(self, next: B) -> Then<Self, B>
    where
        B: Graph<Input = Self::Output, Error = Self::Error>,
    {
        Then(self, next)
    }
    fn pair<B: Graph<Error = Self::Error>>(self, other: B) -> Pair<Self, B> {
        Pair(self, other)
    }
}
