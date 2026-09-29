use super::{Survey, Topology};
use crate::{Facts, Id};
use std::collections::BTreeSet;

pub(super) struct Change {
    pub direct: BTreeSet<Id>,
    pub survey: Survey,
    pub topology: Topology,
}

impl Change {
    pub fn between(current: &Facts, previous: &Facts) -> Self {
        let mut direct = BTreeSet::new();
        let mut stable = current.entities().len() == previous.entities().len();
        let mut aligned = stable;
        for (slot, entity) in current.entities().iter().enumerate() {
            let Some(oldslot) = previous.slot(entity.id) else {
                stable = false;
                aligned = false;
                direct.insert(entity.id);
                continue;
            };
            aligned &= oldslot == slot;
            let old = &previous.entities()[oldslot];
            if (old.version, old.parent, old.source, old.role, old.boundary)
                != (
                    entity.version,
                    entity.parent,
                    entity.source,
                    entity.role,
                    entity.boundary,
                )
            {
                direct.insert(entity.id);
            }
        }
        let mut compared = 0;
        let same = current.edges().len() == previous.edges().len()
            && current
                .edges()
                .iter()
                .zip(previous.edges())
                .all(|(left, right)| {
                    compared += 1;
                    left == right
                });
        let mut indexed = 0;
        if !same {
            let before = previous.edges().iter().copied().collect::<BTreeSet<_>>();
            let after = current.edges().iter().copied().collect::<BTreeSet<_>>();
            indexed = previous.edges().len() + current.edges().len();
            for edge in before.symmetric_difference(&after) {
                direct.extend(
                    [edge.from, edge.to]
                        .into_iter()
                        .filter(|id| current.slot(*id).is_some()),
                );
            }
        }
        let topology = match (stable, aligned, same) {
            (true, true, true) => Topology::Same,
            (true, true, false) => Topology::Aligned,
            (true, false, _) => Topology::Remapped,
            (false, _, _) => Topology::Changed,
        };
        Self {
            direct,
            survey: Survey {
                compared: current.entities().len(),
                topology: compared,
                indexed,
                routed: 0,
            },
            topology,
        }
    }
}
