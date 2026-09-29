mod binding;
mod request;
mod wrappers;
pub use request::Requests;
mod editing;
mod focus;
mod keyboard;
mod shortcuts;
pub use keyboard::input;
pub(crate) mod event;
use crate::Fault;
use domengine::style::animation::Animations;
use domengine::{Document, Node, Sheet};
pub use editing::{compose, edit, transition};
pub use event::message;
pub use focus::{change, click};
use jsengine::{Engine, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

const VALIDATION: shared::cache::Policy = shared::cache::Policy {
    read: true,
    write: true,
    entries: 256,
    bytes: 1_048_576,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bindings {
    pub finds: u64,
    pub elements: u64,
    pub texts: u64,
    pub attributes: u64,
    pub appends: u64,
    pub removes: u64,
    pub replaces: u64,
    pub others: u64,
}

impl Bindings {
    fn record(&mut self, native: &Native) {
        let value = match native {
            Native::Find => &mut self.finds,
            Native::Element => &mut self.elements,
            Native::Text => &mut self.texts,
            Native::Attribute(_) => &mut self.attributes,
            Native::Append(_) => &mut self.appends,
            Native::Remove(_) => &mut self.removes,
            Native::Replace(_) => &mut self.replaces,
            _ => &mut self.others,
        };
        *value = value.saturating_add(1);
    }

    pub fn since(self, previous: Self) -> Self {
        Self {
            finds: self.finds.saturating_sub(previous.finds),
            elements: self.elements.saturating_sub(previous.elements),
            texts: self.texts.saturating_sub(previous.texts),
            attributes: self.attributes.saturating_sub(previous.attributes),
            appends: self.appends.saturating_sub(previous.appends),
            removes: self.removes.saturating_sub(previous.removes),
            replaces: self.replaces.saturating_sub(previous.replaces),
            others: self.others.saturating_sub(previous.others),
        }
    }

    pub fn total(&self) -> u64 {
        self.finds
            .saturating_add(self.elements)
            .saturating_add(self.texts)
            .saturating_add(self.attributes)
            .saturating_add(self.appends)
            .saturating_add(self.removes)
            .saturating_add(self.replaces)
            .saturating_add(self.others)
    }
}

#[derive(Clone)]
enum Native {
    Method(fn(Node) -> Native),
    Fetch(usize),
    Requests,
    Cancel(usize),
    Body(usize, request::Body),
    Register,
    Rebind(crate::keyboard::Context, crate::keyboard::Handle),
    Dispose(crate::keyboard::Context, crate::keyboard::Handle),
    Find,
    Element,
    Text,
    Post,
    Receive,
    Control(event::Lease, event::Control),
    Scope,
    Release(u64),
    Unlisten(u64),
    Dispatch(Node),
    Focused,
    Editor,
    Focus(Node),
    Value(Node),
    Assign(Node),
    Caret(Node),
    Select(Node),
    Scroll,
    Follow(u64, Node),
    Restore(u64, Node),
    Unscroll(u64, Node),
    Visual,
    Unvisual(u64, Node),
    Attribute(Node),
    Children(Node),
    Tag(Node),
    Read(Node),
    Computed,
    Keyframes,
    Version(u64),
    Opacity(u64),
    Change(u64),
    Get(Node),
    Put(Node),
    Append(Node),
    Remove(Node),
    Replace(Node),
    Listen(Node),
}

#[derive(Clone)]
struct Validated;

pub struct Web {
    validation: shared::cache::Cache<String, Validated>,
    prototype: Option<Value>,
    fetch: request::Fetch,
    keyboard: crate::keyboard::Keyboard,
    pub document: Document,
    sheet: Sheet,
    rules: HashMap<u64, Value>,
    animations: Arc<Mutex<Animations>>,
    clock: std::time::Instant,
    bindings: Vec<Option<Native>>,
    vacant: Vec<usize>,
    calls: Bindings,
    wrappers: wrappers::Wrappers,
    subscriptions: event::Registry,
    scopes: HashMap<usize, u64>,
    owners: HashMap<u64, usize>,
    pub(crate) events: event::Dispatches,
    receiver: Option<Value>,
    outgoing: VecDeque<String>,
    composition: Option<(domengine::ime::Id, Node)>,
    pub(crate) scrolls: binding::scroll::Scrolls,
    visuals: binding::visual::Visuals,
}

impl Web {
    pub(crate) fn watching(&self) -> Vec<Node> {
        self.subscriptions.nodes("scroll")
    }

    pub(crate) fn sheet(&self) -> &Sheet {
        &self.sheet
    }
    pub fn new(
        document: Document,
        styling: (Sheet, Arc<Mutex<Animations>>, std::time::Instant),
        config: config::Store,
        caches: &shared::cache::Manager,
    ) -> Result<Self, Fault> {
        let (sheet, animations, clock) = styling;
        Ok(Self {
            validation: caches
                .register("declarations", VALIDATION)
                .map_err(crate::error)?,
            prototype: None,
            document,
            sheet,
            rules: HashMap::new(),
            animations,
            clock,
            fetch: Default::default(),
            keyboard: crate::keyboard::Keyboard::default(),
            bindings: Vec::new(),
            vacant: Vec::new(),
            calls: Default::default(),
            wrappers: wrappers::Wrappers::default(),
            subscriptions: event::Registry::new(config),
            scopes: HashMap::new(),
            owners: HashMap::new(),
            events: event::Dispatches::default(),
            receiver: None,
            outgoing: VecDeque::new(),
            composition: None,
            scrolls: Default::default(),
            visuals: Default::default(),
        })
    }

    fn bind(&mut self, engine: &mut Engine, native: Native) -> Value {
        let index = if let Some(index) = self.vacant.pop() {
            self.bindings[index] = Some(native);
            index
        } else {
            let index = self.bindings.len();
            self.bindings.push(Some(native));
            index
        };
        engine.native(index)
    }

    fn wrap(&mut self, engine: &mut Engine, node: Node) -> Result<Value, Fault> {
        if let Some(value) = self.wrappers.get(&node) {
            return Ok(value.clone());
        }
        let value = engine.derive(self.prototype.as_ref().expect("installed DOM prototype"))?;
        let get = self.bind(engine, Native::Get(node));
        let put = self.bind(engine, Native::Put(node));
        let style = engine.object(
            [
                ("getPropertyValue".into(), get),
                ("setProperty".into(), put),
            ]
            .into(),
        )?;
        engine.set(&value, "style", style).map_err(crate::error)?;
        self.wrappers.insert(node, value.clone());
        Ok(value)
    }

    fn prototype(&mut self, engine: &mut Engine) -> Result<(), Fault> {
        let fields = [
            ("setAttribute", Native::Method(Native::Attribute)),
            ("getAttribute", Native::Method(Native::Read)),
            ("children", Native::Method(Native::Children)),
            ("tagName", Native::Method(Native::Tag)),
            ("appendChild", Native::Method(Native::Append)),
            ("removeChild", Native::Method(Native::Remove)),
            ("setText", Native::Method(Native::Replace)),
            ("addEventListener", Native::Method(Native::Listen)),
            ("dispatchEvent", Native::Method(Native::Dispatch)),
            ("focus", Native::Method(Native::Focus)),
            ("getValue", Native::Method(Native::Value)),
            ("setValue", Native::Method(Native::Assign)),
            ("setCaret", Native::Method(Native::Caret)),
            ("setSelection", Native::Method(Native::Select)),
        ]
        .into_iter()
        .map(|(name, native)| (name.into(), self.bind(engine, native)))
        .collect();
        self.prototype = Some(engine.object(fields)?);
        Ok(())
    }

    fn node(&self, value: &Value) -> Result<Node, Fault> {
        self.wrappers.node(value)
    }

    pub fn messages(&mut self) -> Vec<String> {
        self.outgoing.drain(..).collect()
    }
    pub fn bindings(&self) -> Bindings {
        self.calls
    }
}

fn text(value: &Value) -> Result<&str, Fault> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(Fault("expected text".into())),
    }
}
