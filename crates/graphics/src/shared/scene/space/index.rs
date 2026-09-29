use super::{Mapping, Space, Spatial};
use crate::shared::{Fault, Visual};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Index {
    entries: Vec<Entry>,
    indices: HashMap<u64, usize>,
}
#[derive(Debug)]
struct Entry {
    space: Space,
    opacity: u8,
    parent: usize,
    scroller: u64,
    descendants: usize,
}
impl Index {
    pub fn len(&self) -> usize {
        self.entries.len() + 1
    }
    pub(in crate::shared::scene) fn project(nodes: &[Spatial]) -> Result<Self, Fault> {
        let mut index = Self::default();
        let mut depths = vec![0usize];
        for (slot, node) in nodes.iter().enumerate() {
            if node.parent > slot || node.descendants > nodes.len() - slot - 1 {
                return Err(Fault("invalid spatial topology".into()));
            }
            let depth = depths[node.parent] + 1;
            if depth > 32 {
                return Err(Fault("scene depth budget exhausted".into()));
            }
            super::super::validate::spacecheck(&node.space)?;
            index.insert(&node.space, node.opacity, node.parent)?;
            index.entries[slot].descendants = node.descendants;
            depths.push(depth);
        }
        for (slot, entry) in index.entries.iter().enumerate() {
            if entry.parent > 0
                && index.entries[entry.parent - 1].descendants < slot + 1 - entry.parent
            {
                return Err(Fault("spatial parent does not contain its child".into()));
            }
        }
        Ok(index)
    }
    pub fn insert(&mut self, space: &Space, opacity: u8, parent: usize) -> Result<usize, Fault> {
        let index = self.entries.len() + 1;
        if space.id == 0 || self.indices.insert(space.id, index).is_some() {
            return Err(Fault("duplicate space identity".into()));
        }
        let scroller = if space.scroll.is_some() {
            space.id
        } else {
            parent
                .checked_sub(1)
                .map_or(0, |index| self.entries[index].scroller)
        };
        self.entries.push(Entry {
            space: space.clone(),
            opacity,
            parent,
            scroller,
            descendants: 0,
        });
        Ok(index)
    }
    pub fn finish(&mut self, index: usize) {
        self.entries[index - 1].descendants = self.entries.len() - index;
    }
    pub fn descendants(&self, id: u64) -> usize {
        self.entries[self.indices[&id] - 1].descendants
    }
    pub fn get(&self, id: u64) -> Option<usize> {
        if id == 0 {
            Some(0)
        } else {
            self.indices.get(&id).copied()
        }
    }
    pub fn opacity(&self, id: u64) -> Option<u8> {
        self.get(id)
            .and_then(|index| index.checked_sub(1))
            .map(|index| self.entries[index].opacity)
    }
    pub fn range(&self, id: u64) -> Option<usize> {
        self.get(id)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.entries[index].space.scroll)
    }
    pub fn scroller(&self, id: u64) -> Option<u64> {
        self.get(id).map(|index| {
            index
                .checked_sub(1)
                .map_or(0, |index| self.entries[index].scroller)
        })
    }
    pub fn resolve(&self, root: Mapping, visual: &Visual) -> Result<Vec<Mapping>, Fault> {
        let mut mappings = Vec::with_capacity(self.entries.len() + 1);
        mappings.push(root);
        for entry in &self.entries {
            mappings
                .push(mappings[entry.parent].child(&entry.space, visual.offset(entry.space.id))?);
        }
        Ok(mappings)
    }
    pub fn affected(&self, offsets: &[u64]) -> usize {
        let mut intervals = Vec::with_capacity(offsets.len());
        for id in offsets {
            if *id == 0 {
                return self.len();
            }
            let Some(index) = self.indices.get(id).copied() else {
                continue;
            };
            intervals.push((index, index + self.entries[index - 1].descendants + 1));
        }
        intervals.sort_unstable_by_key(|(start, _)| *start);
        let mut affected = 0;
        let mut end = 0;
        for (start, next) in intervals {
            if next <= end {
                continue;
            }
            affected += next - start.max(end);
            end = next;
        }
        affected
    }
}
