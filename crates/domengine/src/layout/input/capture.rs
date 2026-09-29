mod children;
use super::{Element, Input, Update, damage::Damage, element::Context};
use crate::{
    Document, Fault, Node, Sheet,
    style::{Inherited, Resolver, Subject, animation::Animations},
};
use children::{Children, Lineage};
use std::sync::Arc;
use std::sync::Mutex;

pub struct Capture {
    resolver: Resolver,
    animations: Arc<Mutex<Animations>>,
    animated: std::collections::BTreeSet<Node>,
    committed: Option<Input>,
    enabled: bool,
    reset: Option<String>,
    report: Update,
}
impl Capture {
    pub fn new(resolver: Resolver) -> Self {
        Self {
            resolver,
            animations: Arc::new(Mutex::new(Animations::default())),
            animated: std::collections::BTreeSet::new(),
            committed: None,
            enabled: true,
            reset: Some("initial".into()),
            report: Update::default(),
        }
    }
    pub fn replace(&mut self, sheet: Sheet) {
        self.resolver.replace(sheet);
        self.clear();
        self.reset = Some("stylesheet".into());
    }
    pub fn advance(&mut self, now: u64) -> bool {
        !self.tick(now).is_empty()
    }
    pub fn tick(&mut self, now: u64) -> Vec<crate::style::animation::Change> {
        let mut animations = self.animations.lock().expect("animation state");
        animations.advance(now);
        let changes = animations.changes();
        for change in &changes {
            self.animated.insert(change.node);
        }
        changes
    }
    pub fn animating(&self) -> bool {
        self.animations.lock().expect("animation state").running()
    }
    pub fn schedule(&self, now: u64) -> Option<crate::style::animation::Schedule> {
        self.animations
            .lock()
            .expect("animation state")
            .schedule(now)
    }
    pub fn animations(&self) -> Arc<Mutex<Animations>> {
        self.animations.clone()
    }
    pub fn plans(&self) -> Vec<crate::style::animation::Track<Node>> {
        self.animations.lock().expect("animation state").plans()
    }
    pub fn clear(&mut self) {
        self.committed = None;
        self.report.committed = None;
        self.reset = Some("cleared".into());
    }
    pub fn close(&mut self) {
        self.clear();
        self.resolver.close();
    }
    pub fn maintain(&mut self) {
        self.resolver.maintain();
    }
    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn inspect(&self) -> Update {
        self.report.clone()
    }
    pub fn update<T>(
        &mut self,
        document: &Document,
        publish: impl FnOnce(&Input) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        self.maintain();
        let start = std::time::Instant::now();
        let input = observe::call("dom.capture", || self.prepare(document));
        self.report.elapsed = start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.resolver.publish();
        let result = input.and_then(|input| {
            let output = publish(&input)?;
            self.report.committed = Some(input.revision);
            self.committed = Some(input);
            self.reset = None;
            self.animated.clear();
            self.animations.lock().expect("animation state").discard();
            Ok(output)
        });
        self.report.failure = result.as_ref().err().map(ToString::to_string);
        result
    }
    fn prepare(&mut self, document: &Document) -> Result<Input, Fault> {
        self.report = Update {
            history: document.retention(),
            enabled: self.enabled,
            revision: document.revision(),
            committed: self.committed.as_ref().map(|input| input.revision),
            reset: self.reset.clone(),
            ..Update::default()
        };
        let previous = self.committed.as_ref();
        let history = previous.map(|input| input.tree.clone());
        let changes = previous.and_then(|input| document.changes(input.version()));
        if previous.is_some() && changes.is_none() {
            self.report.reset = Some(
                if previous.is_some_and(|input| input.tree.element().node != document.root()) {
                    "document"
                } else {
                    "history"
                }
                .into(),
            );
        }
        if !self.enabled {
            self.report.reset = Some("disabled".into());
        }
        let changes = changes.unwrap_or_default();
        self.report.changes = changes.len();
        if self.enabled
            && self.report.reset.is_none()
            && previous.is_some_and(|input| {
                changes
                    .len()
                    .saturating_mul(std::mem::size_of::<crate::change::Change>())
                    >= input
                        .tree
                        .element()
                        .count
                        .saturating_mul(std::mem::size_of::<Element>())
            })
        {
            self.report.reset = Some("changes".into());
        }
        let mut damage = if self.report.reset.is_none() {
            Damage::collect(document, &changes)?
        } else {
            Damage::default()
        };
        if self.report.reset.is_none() {
            damage.animate(document, self.animated.iter().copied())?;
        }
        self.report.affected = damage.nodes.len();
        self.report.ancestry = damage.ancestry;
        let lineage = Lineage::new(previous.map(|input| input.tree.element()));
        let previous = previous
            .filter(|_| self.report.reset.is_none())
            .map(|input| input.tree.element());
        let mut animations = self.animations.lock().expect("animation state");
        let mut build = Build {
            document,
            resolver: &mut self.resolver,
            animations: &mut animations,
            damage: &damage,
            report: &mut self.report,
            lineage,
        };
        let root = build.node(
            document.root(),
            previous,
            build.lineage.find(document.root()),
            Inherited::default(),
        )?;
        animations.retain(|node| document.connected(node).unwrap_or(false));
        self.report.nodes = root.count;
        let revision = document.revision();
        let draft = if self.report.reset.is_some() {
            super::damage::Draft::full()
        } else {
            damage.draft()
        };
        let (tree, timing) = super::Tree::build(&root, revision, draft, history.as_deref());
        self.report.timing = timing;
        Ok(Input { revision, tree })
    }
}
struct Build<'a> {
    document: &'a Document,
    resolver: &'a mut Resolver,
    animations: &'a mut Animations,
    damage: &'a Damage,
    report: &'a mut Update,
    lineage: Lineage<'a>,
}
impl Build<'_> {
    fn node(
        &mut self,
        node: Node,
        previous: Option<&Arc<Element>>,
        identity: Option<&Arc<Element>>,
        inherited: Inherited,
    ) -> Result<Arc<Element>, Fault> {
        self.report.visited += 1;
        if let Some(old) = previous
            && old.inherited == inherited
            && !self.damage.nodes.contains(&node)
        {
            self.report.reused += 1;
            self.report.skipped += old.count - 1;
            return Ok(old.clone());
        }
        let style = match previous
            .filter(|old| old.inherited == inherited && !self.damage.styles.contains(&node))
        {
            Some(old) if old.style.animation.name.is_none() => old.style.clone(),
            _ => {
                self.report.styled += 1;
                self.resolver
                    .resolve(Subject::capture(self.document, node)?, inherited.clone())?
            }
        };
        let style = if let Some(name) = style.animation.name.as_deref()
            && let Some(frames) = self.resolver.keyframes(name)
        {
            let value = self
                .animations
                .sample(node, &style.animation, frames, style.opacity);
            if value == style.opacity {
                style
            } else {
                let mut animated = (*style).clone();
                animated.opacity = value;
                Arc::new(animated)
            }
        } else {
            self.animations.cancel(node);
            style
        };
        let mut old = Children::new(previous);
        let children = self
            .document
            .entry(node)?
            .children
            .iter()
            .map(|child| {
                self.node(
                    *child,
                    old.find(*child),
                    self.lineage.find(*child),
                    Inherited::from(style.as_ref()),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let element = Element::finish(
            self.document,
            node,
            Context {
                style,
                inherited,
                previous,
                identity,
            },
            children,
        )?;
        self.report.rebuilt += 1;
        self.report.projected += 1 + usize::from(matches!(
            &self.document.entry(node)?.kind,
            crate::Kind::Element(tag) if tag == "input"
        ));
        Ok(Arc::new(element))
    }
}
