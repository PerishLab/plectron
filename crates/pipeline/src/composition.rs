use crate::{Graph, Node, Observer, Topology};

pub struct Step<N>(pub(crate) N);
pub struct Then<A, B>(pub(crate) A, pub(crate) B);
pub struct Pair<A, B>(pub(crate) A, pub(crate) B);
impl<N> crate::sealed::Sealed for Step<N> {}
impl<A, B> crate::sealed::Sealed for Then<A, B> {}
impl<A, B> crate::sealed::Sealed for Pair<A, B> {}

impl<N: Node> Graph for Step<N> {
    type Input = N::Input;
    type Output = N::Output;
    type Error = N::Error;
    const LEN: usize = 1;
    fn evaluate(
        &mut self,
        input: Self::Input,
        observer: &mut impl Observer,
        offset: usize,
    ) -> Result<Self::Output, Self::Error> {
        observer.node((offset, N::NAME), || {
            observe::node((offset, N::NAME), || self.0.run(input))
        })
    }
    fn describe(&self, topology: &mut Topology, incoming: &[usize]) -> Vec<usize> {
        vec![topology.append(N::NAME, incoming)]
    }
}

impl<A, B> Graph for Then<A, B>
where
    A: Graph,
    B: Graph<Input = A::Output, Error = A::Error>,
{
    type Input = A::Input;
    type Output = B::Output;
    type Error = A::Error;
    const LEN: usize = A::LEN + B::LEN;
    fn evaluate(
        &mut self,
        input: Self::Input,
        observer: &mut impl Observer,
        offset: usize,
    ) -> Result<Self::Output, Self::Error> {
        let output = self.0.evaluate(input, observer, offset)?;
        self.1.evaluate(output, observer, offset + A::LEN)
    }
    fn describe(&self, topology: &mut Topology, incoming: &[usize]) -> Vec<usize> {
        let tails = self.0.describe(topology, incoming);
        self.1.describe(topology, &tails)
    }
}

impl<A: Graph, B: Graph<Error = A::Error>> Graph for Pair<A, B> {
    type Input = (A::Input, B::Input);
    type Output = (A::Output, B::Output);
    type Error = A::Error;
    const LEN: usize = A::LEN + B::LEN;
    fn evaluate(
        &mut self,
        input: Self::Input,
        observer: &mut impl Observer,
        offset: usize,
    ) -> Result<Self::Output, Self::Error> {
        let left = self.0.evaluate(input.0, observer, offset)?;
        let right = self.1.evaluate(input.1, observer, offset + A::LEN)?;
        Ok((left, right))
    }
    fn describe(&self, topology: &mut Topology, incoming: &[usize]) -> Vec<usize> {
        let mut tails = self.0.describe(topology, incoming);
        tails.extend(self.1.describe(topology, incoming));
        tails
    }
}
