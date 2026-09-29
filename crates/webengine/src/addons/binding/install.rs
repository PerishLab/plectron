use super::super::{Native, Web};
use crate::Fault;
use jsengine::Engine;
use jsruntime::Capabilities;

impl jsruntime::Addons for Web {
    fn install(&mut self, engine: &mut Engine, host: &mut Capabilities) -> Result<(), Fault> {
        self.prototype(engine)?;
        let fields = [
            ("getElementById", Native::Find),
            ("getComputedStyle", Native::Computed),
            ("keyframes", Native::Keyframes),
            ("createElement", Native::Element),
            ("createTextNode", Native::Text),
            ("focused", Native::Focused),
            ("attachEditor", Native::Editor),
        ]
        .into_iter()
        .map(|(name, native)| (name.into(), self.bind(engine, native)))
        .collect();
        let document = engine.object(fields)?;
        let root = self.wrap(engine, self.document.root())?;
        engine.set(&document, "root", root).map_err(crate::error)?;
        host.insert("document".into(), document);

        let fields = [
            ("postMessage", Native::Post),
            ("onMessage", Native::Receive),
        ]
        .into_iter()
        .map(|(name, native)| (name.into(), self.bind(engine, native)))
        .collect();
        host.insert("bridge".into(), engine.object(fields)?);
        host.insert("shortcuts".into(), self.shortcuts(engine)?);
        let (fetch, requests) = self.network(engine)?;
        host.insert("fetch".into(), fetch);
        host.insert("requests".into(), requests);

        let scope = self.bind(engine, Native::Scope);
        let events = engine.object([("scope".into(), scope)].into())?;
        engine.freeze(&events)?;
        host.insert("events".into(), events);
        let attach = self.bind(engine, Native::Scroll);
        let scroll = engine.object([("attach".into(), attach)].into())?;
        engine.freeze(&scroll)?;
        host.insert("scroll".into(), scroll);
        let observe = self.bind(engine, Native::Visual);
        let visual = engine.object([("observe".into(), observe)].into())?;
        engine.freeze(&visual)?;
        host.insert("visual".into(), visual);
        Ok(())
    }
}
