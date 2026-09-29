use crate::scope::Scope;
use crate::syntax::Function;
use crate::{Addons, Engine, Fault, Handle};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Collection {
    pub live: usize,
    pub released: usize,
    pub capacity: usize,
    pub vacant: usize,
    pub weak: usize,
    pub edges: usize,
    pub activations: usize,
    pub rounds: usize,
}

impl Engine {
    pub fn collect(&mut self, host: &mut impl Addons) -> Result<usize, Fault> {
        observe::call("js.collect", || self.reclaim(host))
    }

    fn reclaim(&mut self, host: &mut impl Addons) -> Result<usize, Fault> {
        let reachable = self.reachability(host)?;
        let dead: Vec<Handle> = self
            .live
            .iter()
            .enumerate()
            .filter(|(slot, live)| {
                **live
                    && !reachable.objects.contains(&Handle {
                        owner: self.identity,
                        slot: *slot,
                        epoch: self.epochs[*slot],
                    })
            })
            .map(|(slot, _)| Handle {
                owner: self.identity,
                slot,
                epoch: self.epochs[slot],
            })
            .collect();
        self.weak.sweep(&reachable.objects);
        host.sweep(&dead, &|binding| reachable.natives.contains(&binding));
        self.promises.prune(&dead, &reachable.resolvers);
        for handle in &dead {
            self.objects[handle.slot] = crate::value::Object::new(Default::default());
            self.live[handle.slot] = false;
            self.vacant.push(handle.slot);
        }
        for (slot, scope) in self.scopes.iter_mut().enumerate().skip(1) {
            if self.arenas.scopes.contains(slot) && !reachable.scopes.contains(&slot) {
                *scope = Scope::default();
                self.arenas.scopes.release(slot);
            }
        }
        for (slot, binding) in self.bindings.iter_mut().enumerate() {
            if self.arenas.bindings.contains(slot) && !reachable.bindings.contains(&slot) {
                binding.value = None;
                self.arenas.bindings.release(slot);
            }
        }
        for (slot, closure) in self.closures.iter_mut().enumerate() {
            if self.arenas.closures.contains(slot) && !reachable.closures.contains(&slot) {
                closure.code = Function::default();
                closure.home = None;
                closure.scope = 0;
                self.arenas.closures.release(slot);
            }
        }
        self.jit.prune(&reachable.closures);
        let (weak, edges) = self.weak.shape();
        self.collection = Collection {
            live: reachable.objects.len(),
            released: dead.len(),
            capacity: self.objects.len(),
            vacant: self.vacant.len(),
            weak,
            edges,
            activations: reachable.activations,
            rounds: reachable.rounds,
        };
        Ok(dead.len())
    }
}
