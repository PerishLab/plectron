use super::{Plan, Survey};
use crate::Id;
use std::{collections::BTreeSet, sync::Arc};

#[derive(Debug, Clone)]
pub struct Impact {
    seeds: Arc<[Id]>,
    affected: Arc<[Id]>,
    missed: Arc<[Id]>,
    excess: Arc<[Id]>,
    survey: Survey,
}

impl Impact {
    pub fn audit(plan: Plan, changed: &[Id]) -> Self {
        let actual = changed.iter().copied().collect::<BTreeSet<_>>();
        let predicted = plan.affected.iter().copied().collect::<BTreeSet<_>>();
        Self {
            seeds: plan.seeds,
            affected: plan.affected,
            missed: actual.difference(&predicted).copied().collect(),
            excess: predicted.difference(&actual).copied().collect(),
            survey: plan.survey,
        }
    }

    pub fn seeds(&self) -> &[Id] {
        &self.seeds
    }

    pub fn affected(&self) -> &[Id] {
        &self.affected
    }

    pub fn missed(&self) -> &[Id] {
        &self.missed
    }

    pub fn excess(&self) -> &[Id] {
        &self.excess
    }

    pub const fn survey(&self) -> Survey {
        self.survey
    }
}
