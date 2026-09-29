mod dispatch;
use super::convert::{integer, text};
use crate::contract::{Event, Extent, Signal, Unit, Window};
use crate::platform::Host;
use crate::runtime::{Addons, Capabilities};
pub use dispatch::{Dispatch, Pump};
use jsengine::{Engine, Fault, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum Native {
    Window,
    Serve,
    Scheme(u64),
    Stop(u64),
    Load(Unit),
    Message(Unit),
    Log,
    Title(Unit),
    Fill(Unit),
    On(Unit),
    Close(Unit),
}

pub struct Control<H> {
    pub host: H,
    natives: Vec<Option<Native>>,
    vacant: Vec<usize>,
    listeners: BTreeMap<(Unit, String), Vec<Value>>,
}

impl<H: Host> Control<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            natives: Vec::new(),
            vacant: Vec::new(),
            listeners: BTreeMap::new(),
        }
    }

    fn serve(&mut self, engine: &mut Engine, arguments: &[Value]) -> Result<Value, Fault> {
        let [value] = arguments else {
            return Err(Fault("serve expects a directory".into()));
        };
        let service = self.host.serve(&text(value)?)?;
        let fields = BTreeMap::from([
            ("scheme".into(), self.bind(engine, Native::Scheme(service))),
            ("close".into(), self.bind(engine, Native::Stop(service))),
        ]);
        engine.object(fields)
    }

    fn bind(&mut self, engine: &mut Engine, native: Native) -> Value {
        let binding = if let Some(binding) = self.vacant.pop() {
            self.natives[binding] = Some(native);
            binding
        } else {
            let binding = self.natives.len();
            self.natives.push(Some(native));
            binding
        };
        engine.native(binding)
    }

    fn window(&mut self, engine: &mut Engine, arguments: &[Value]) -> Result<Value, Fault> {
        let [value] = arguments else {
            return Err(Fault("window expects one options object".into()));
        };
        let options = engine.properties(value)?;
        for key in options.keys() {
            if !matches!(key.as_str(), "title" | "width" | "height") {
                return Err(Fault(format!("unknown window option: {key}")));
            }
        }
        let title = text(
            options
                .get("title")
                .unwrap_or(&Value::Text("Plectron".into())),
        )?;
        let width = integer(options.get("width").unwrap_or(&Value::Number(640.0)), 4096)?;
        let height = integer(options.get("height").unwrap_or(&Value::Number(480.0)), 4096)?;
        let unit = self.host.window(Window {
            title,
            extent: Extent { width, height },
        })?;
        let fields = BTreeMap::from([
            ("title".into(), self.bind(engine, Native::Title(unit))),
            ("fill".into(), self.bind(engine, Native::Fill(unit))),
            ("on".into(), self.bind(engine, Native::On(unit))),
            ("close".into(), self.bind(engine, Native::Close(unit))),
            ("load".into(), self.bind(engine, Native::Load(unit))),
            (
                "postMessage".into(),
                self.bind(engine, Native::Message(unit)),
            ),
        ]);
        engine.object(fields)
    }

    fn listen(&mut self, engine: &Engine, unit: Unit, arguments: &[Value]) -> Result<Value, Fault> {
        self.host.check(unit)?;
        let [Value::Text(name), callback] = arguments else {
            return Err(Fault("on expects an event name and function".into()));
        };
        engine.callable(callback)?;
        if !matches!(
            name.as_str(),
            "click" | "close" | "ready" | "error" | "message"
        ) {
            return Err(Fault(format!("unsupported event: {name}")));
        }
        self.listeners
            .entry((unit, name.clone()))
            .or_default()
            .push(callback.clone());
        Ok(Value::Undefined)
    }

    fn mutate(&mut self, native: Native, arguments: &[Value]) -> Result<Value, Fault> {
        match (native, arguments) {
            (Native::Title(unit), [value]) => self.host.title(unit, &text(value)?)?,
            (Native::Fill(unit), [value]) => {
                self.host.fill(unit, integer(value, 0x00ff_ffff)? as u32)?
            }
            (Native::Close(unit), []) => self.host.close(unit)?,
            (Native::Load(unit), [value]) => self.host.attach(unit, &text(value)?)?,
            (Native::Message(unit), [value]) => self.host.message(unit, &text(value)?)?,
            (Native::Scheme(service), [value]) => self.host.scheme(service, &text(value)?)?,
            (Native::Stop(service), []) => self.host.stop(service)?,
            _ => return Err(Fault("invalid native call arguments".into())),
        }
        Ok(Value::Undefined)
    }

    fn callbacks(&mut self, signal: &Signal) -> Vec<Value> {
        let callbacks = self
            .listeners
            .get(&(signal.unit, signal.event.name().into()))
            .cloned()
            .unwrap_or_default();
        if signal.event == Event::Close {
            self.listeners.retain(|(unit, _), _| *unit != signal.unit);
        } else if self.host.check(signal.unit).is_err() {
            return Vec::new();
        }
        callbacks
    }
}

impl<H: Host> Addons for Control<H> {
    fn install(
        &mut self,
        engine: &mut Engine,
        capabilities: &mut Capabilities,
    ) -> Result<(), Fault> {
        let window = self.bind(engine, Native::Window);
        let log = self.bind(engine, Native::Log);
        let serve = self.bind(engine, Native::Serve);
        let platform = engine.object(BTreeMap::from([
            ("window".into(), window),
            ("serve".into(), serve),
        ]))?;
        let console = engine.object(BTreeMap::from([("log".into(), log)]))?;
        capabilities.insert("platform".into(), platform);
        capabilities.insert("console".into(), console);
        Ok(())
    }
}

impl<H: Host> jsengine::Addons for Control<H> {
    fn sweep(&mut self, _: &[jsengine::Handle], live: &dyn Fn(usize) -> bool) {
        for (id, native) in self.natives.iter_mut().enumerate() {
            if native.is_some() && !live(id) {
                *native = None;
                self.vacant.push(id);
            }
        }
    }

    fn roots(&self, visit: &mut dyn FnMut(&Value)) {
        for callbacks in self.listeners.values() {
            for callback in callbacks {
                visit(callback);
            }
        }
    }

    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, jsengine::Failure> {
        Ok(self.invoke(engine, binding, invocation.arguments)?)
    }
}

impl<H: Host> Control<H> {
    fn invoke(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        arguments: Vec<Value>,
    ) -> Result<Value, Fault> {
        let native = *self
            .natives
            .get(binding)
            .and_then(Option::as_ref)
            .ok_or_else(|| Fault("unknown native binding".into()))?;
        match native {
            Native::Window => self.window(engine, &arguments),
            Native::Serve => self.serve(engine, &arguments),
            Native::Log => {
                self.host.log(
                    &arguments
                        .iter()
                        .map(Value::text)
                        .collect::<Vec<_>>()
                        .join(" "),
                )?;
                Ok(Value::Undefined)
            }
            Native::On(unit) => self.listen(engine, unit, &arguments),
            _ => self.mutate(native, &arguments),
        }
    }
}
