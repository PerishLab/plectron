use super::{super::envelope::Envelope, Trees};
use crate::shared::{Fault, Item, Output};
use std::sync::Arc;

pub(super) struct Admission<'a>(pub &'a mut Trees);
pub(super) enum Route {
    Transparent,
    Build,
    Retained(Option<Envelope>),
}
impl<'a> pipeline::Node for Admission<'a> {
    type Input = (&'a Arc<[Item]>, Output, usize);
    type Output = Route;
    type Error = Fault;
    const NAME: &'static str = "render.admission";
    fn run(&mut self, (items, output, descendants): Self::Input) -> Result<Route, Fault> {
        if !self.0.accepts(descendants) {
            return Ok(Route::Transparent);
        }
        Ok(match self.0.get(items, output) {
            Some(envelope) => Route::Retained(envelope),
            None => Route::Build,
        })
    }
}
