use crate::layout::{
    input::{Entry, Key, Origin},
    measure::Measured,
};
use crate::{Branch, Constraints, layout::Snapshot};
use std::sync::Arc;

pub(super) struct Retention {
    pub key: Arc<Key>,
    pub owner: Origin,
    pub root: Arc<Branch>,
    pub measurement: crate::layout::measure::Generation,
    pub constraints: Constraints,
    pub work: usize,
    pub snapshot: Arc<Snapshot>,
    pub projection: crate::layout::delta::Projection,
}
impl Retention {
    pub fn bind<'a>(
        &self,
        element: Entry<'a>,
        constraints: Constraints,
        context: (
            Option<&std::collections::BTreeSet<crate::Node>>,
            Option<&mut super::Estimates>,
        ),
        bindings: &mut crate::layout::measure::snapshot::bindings::Bindings,
    ) -> Result<Option<Measured<'a>>, crate::Fault> {
        let (demand, estimates) = context;
        let measured = (self.constraints == constraints && Arc::ptr_eq(&self.key, &element.key))
            .then(|| self.measurement.root().bind(element));
        let measured = measured.filter(|measured| {
            self.owner.matches(element.record())
                || measured.geometry.accepts(element, demand, estimates)
        });
        if let Some(measured) = &measured {
            self.snapshot.pin(&measured.geometry, bindings)?;
        }
        Ok(measured)
    }
}
