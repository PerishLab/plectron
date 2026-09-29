use crate::{
    Fault,
    layout::{fragment::builder::Builder, measure::View},
};

pub(super) struct Glyphs<'a, 'b>(pub &'a mut Builder<'b>);
impl<'a> pipeline::Node for Glyphs<'a, '_> {
    type Input = (&'a View<'a>, usize, usize);
    type Output = ();
    type Error = Fault;
    const NAME: &'static str = "text.materialization";
    fn run(&mut self, (measured, line, parent): Self::Input) -> Result<(), Fault> {
        self.0.materialize(measured, line, parent)
    }
}
