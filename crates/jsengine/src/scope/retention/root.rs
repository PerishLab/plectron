use crate::{Engine, Fault, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Root {
    owner: u64,
    slot: usize,
    epoch: u64,
}

#[derive(Default)]
pub(crate) struct Registry {
    slots: Vec<Slot>,
    vacant: Vec<usize>,
}

impl Registry {
    pub(crate) fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.slots.iter().filter_map(|slot| slot.value.clone())
    }
}

struct Slot {
    value: Option<Value>,
    epoch: u64,
}

impl Engine {
    pub fn retain(&mut self, value: &Value) -> Result<Root, Fault> {
        self.validate(value)?;
        let slot = if let Some(slot) = self.retained.vacant.pop() {
            let entry = &mut self.retained.slots[slot];
            entry.epoch = entry
                .epoch
                .checked_add(1)
                .ok_or_else(|| Fault("root identity exhausted".into()))?;
            entry.value = Some(value.clone());
            slot
        } else {
            let slot = self.retained.slots.len();
            self.retained.slots.push(Slot {
                value: Some(value.clone()),
                epoch: 0,
            });
            slot
        };
        Ok(Root {
            owner: self.identity,
            slot,
            epoch: self.retained.slots[slot].epoch,
        })
    }

    pub fn held(&self, root: Root) -> Result<Value, Fault> {
        Ok(self.root(root)?.clone())
    }

    pub fn release(&mut self, root: Root) -> Result<(), Fault> {
        self.root(root)?;
        self.retained.slots[root.slot].value = None;
        self.retained.vacant.push(root.slot);
        Ok(())
    }

    fn root(&self, root: Root) -> Result<&Value, Fault> {
        if root.owner != self.identity {
            return Err(Fault("root belongs to another engine".into()));
        }
        self.retained
            .slots
            .get(root.slot)
            .filter(|slot| slot.epoch == root.epoch)
            .and_then(|slot| slot.value.as_ref())
            .ok_or_else(|| Fault("root is released".into()))
    }
}
