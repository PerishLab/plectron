mod install;
mod retention;
pub(crate) mod scroll;
mod styling;
pub(super) mod visual;
use super::{Native, Web, text};
use crate::{Fault, error};
use domengine::style::{self, Inherited};
use jsengine::{Engine, Value};

impl Web {
    pub(super) fn invoke(
        &mut self,
        engine: &mut Engine,
        native: Native,
        arguments: Vec<Value>,
    ) -> Result<Value, Fault> {
        if matches!(
            native,
            Native::Fetch(_) | Native::Requests | Native::Cancel(_) | Native::Body(_, _)
        ) {
            return self.requesting(native, &arguments, engine);
        }
        if matches!(
            &native,
            Native::Get(_)
                | Native::Put(_)
                | Native::Computed
                | Native::Keyframes
                | Native::Version(_)
                | Native::Opacity(_)
                | Native::Change(_)
        ) {
            return self.styling(engine, native, &arguments);
        }
        match (native, arguments.as_slice()) {
            (Native::Register, arguments) => self.register(engine, arguments),
            (Native::Rebind(context, handle), [priority, entries]) => {
                let set = super::shortcuts::parse(engine, priority, entries)?;
                self.keyboard
                    .scope(context)
                    .replace(&handle, set)
                    .map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Dispose(context, handle), []) => {
                self.keyboard
                    .scope(context)
                    .remove(&handle)
                    .map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Find, [value]) => match self.document.find(text(value)?) {
                Some(node) => self.wrap(engine, node),
                None => Ok(Value::Null),
            },
            (Native::Element, [value]) => {
                let node = self.document.element(text(value)?).map_err(error)?;
                self.wrap(engine, node)
            }
            (Native::Text, [value]) => {
                let node = self.document.text(text(value)?).map_err(error)?;
                self.wrap(engine, node)
            }
            (Native::Attribute(node), [name, value]) => self.attribute(node, name, value),
            (Native::Read(node), [name]) => Ok(self
                .document
                .entry(node)
                .map_err(error)?
                .attributes
                .get(text(name)?)
                .map_or(Value::Null, |value| Value::Text(value.clone()))),
            (Native::Tag(node), []) => Ok(match &self.document.entry(node).map_err(error)?.kind {
                domengine::Kind::Element(tag) => Value::Text(tag.clone()),
                domengine::Kind::Text(_) => Value::Null,
            }),
            (Native::Children(node), []) => {
                let children = self.document.entry(node).map_err(error)?.children.clone();
                let values = children
                    .into_iter()
                    .map(|child| self.wrap(engine, child))
                    .collect::<Result<Vec<_>, _>>()?;
                engine
                    .array(values.into_iter().map(Some).collect())
                    .map_err(error)
            }
            (Native::Append(parent), [value]) => {
                let focused = self.document.focused();
                self.document
                    .append(parent, self.node(value)?)
                    .map_err(error)?;
                super::focus::announce(engine, self, focused)?;
                Ok(value.clone())
            }
            (Native::Remove(parent), [value]) => {
                let focused = self.document.focused();
                self.document
                    .remove(parent, self.node(value)?)
                    .map_err(error)?;
                super::focus::announce(engine, self, focused)?;
                Ok(value.clone())
            }
            (Native::Replace(node), [value]) => {
                self.document.replace(node, text(value)?).map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Listen(node), arguments) => self.listening(engine, node, arguments),
            (Native::Dispatch(node), arguments) => self.dispatch(engine, node, arguments),
            (Native::Scope, []) => self.scope(engine),
            (Native::Release(id), []) => {
                self.subscriptions.release(id)?;
                if let Some(handle) = self.owners.remove(&id) {
                    self.scopes.remove(&handle);
                }
                Ok(Value::Undefined)
            }
            (Native::Unlisten(id), []) => {
                self.subscriptions.remove(id)?;
                Ok(Value::Undefined)
            }
            (Native::Post, [value]) => {
                let message = text(value)?;
                if message.len() > 16_384 || self.outgoing.len() >= 256 {
                    return Err(Fault("message budget exhausted".into()));
                }
                self.outgoing.push_back(message.into());
                Ok(Value::Undefined)
            }
            (Native::Receive, [callback]) => {
                engine.callable(callback)?;
                self.receiver = Some(callback.clone());
                Ok(Value::Undefined)
            }
            (Native::Focused, []) => match self.document.focused() {
                Some(node) => self.wrap(engine, node),
                None => Ok(Value::Null),
            },
            (Native::Editor, [host, text, caret]) => {
                self.document
                    .enable(self.node(host)?, self.node(text)?, self.node(caret)?)
                    .map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Scroll, [node]) => {
                let node = self.node(node)?;
                self.scrolling(engine, node)
            }
            (Native::Follow(lease, _), []) => {
                self.scrolls.follow(lease)?;
                Ok(Value::Undefined)
            }
            (Native::Restore(lease, _), [target, inset]) => {
                let target = self.node(target)?;
                self.scrolls.restore(lease, target, displacement(inset)?)?;
                Ok(Value::Undefined)
            }
            (Native::Unscroll(lease, _), []) => {
                self.scrolls.release(lease);
                Ok(Value::Undefined)
            }
            (Native::Visual, arguments) => self.observing(engine, arguments),
            (Native::Unvisual(lease, _), []) => {
                self.visuals.release(lease);
                Ok(Value::Undefined)
            }
            (Native::Focus(node), []) => {
                super::focus::focus(engine, self, Some(node)).map(|()| Value::Undefined)
            }
            (Native::Value(node), []) => Ok(Value::Text(
                self.document.buffer(node).map_err(error)?.text().into(),
            )),
            (Native::Assign(node), [value]) => {
                super::editing::abandon(self);
                self.document.value(node, text(value)?).map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Caret(node), [value]) => {
                super::editing::abandon(self);
                self.document.caret(node, position(value)?).map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Select(node), [anchor, focus]) => {
                super::editing::abandon(self);
                self.document
                    .select(
                        node,
                        domengine::Selection {
                            anchor: position(anchor)?,
                            focus: position(focus)?,
                        },
                    )
                    .map_err(error)?;
                Ok(Value::Undefined)
            }
            (Native::Control(lease, control), []) => {
                Ok(Value::Boolean(self.events.control(lease, control)))
            }
            _ => Err(Fault("invalid Web addon arguments".into())),
        }
    }

    pub(super) fn attribute(
        &mut self,
        node: domengine::Node,
        name: &Value,
        value: &Value,
    ) -> Result<Value, Fault> {
        if text(name)? == "style" {
            let source = text(value)?.to_owned();
            let result = if self.validation.get(&source).is_none() {
                style::declarations(&source)
                    .map(|_| {
                        let bytes = source.len() + std::mem::size_of::<String>();
                        self.validation.insert(source, super::Validated, bytes);
                    })
                    .map_err(error)
            } else {
                Ok(())
            };
            self.validation.publish();
            result?;
        }
        self.document
            .attribute(node, text(name)?, text(value)?)
            .map_err(error)?;
        Ok(Value::Undefined)
    }

    pub(super) fn computed(
        &mut self,
        engine: &mut Engine,
        node: domengine::Node,
    ) -> Result<Value, Fault> {
        let now = self.clock.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        let mut animations = self.animations.lock().expect("animation state");
        animations.advance(now);
        let mut inherited = Inherited::default();
        let mut computed = None;
        for ancestor in self.document.path(node).map_err(error)?.into_iter().rev() {
            let mut style = self
                .sheet
                .compute(&self.document, ancestor, inherited)
                .map_err(error)?;
            if let Some(name) = style.animation.name.as_deref()
                && let Some(frames) = self.sheet.keyframes(name)
            {
                style.opacity =
                    animations.sample(ancestor, &style.animation, frames, style.opacity);
            }
            inherited = Inherited::from(&style);
            computed = Some(style);
        }
        let style = computed.ok_or_else(|| Fault("node has no computed style".into()))?;
        let fields = [
            (
                "opacity".into(),
                Value::Text(format!("{}", f64::from(style.opacity) / 255.0)),
            ),
            ("color".into(), Value::Text(format!("#{:06x}", style.color))),
            (
                "backgroundColor".into(),
                style
                    .background
                    .map_or(Value::Text("transparent".into()), |color| {
                        Value::Text(format!("#{color:06x}"))
                    }),
            ),
            (
                "animationName".into(),
                Value::Text(style.animation.name.unwrap_or_else(|| "none".into())),
            ),
        ];
        engine.object(fields.into()).map_err(error)
    }
}

fn position(value: &Value) -> Result<usize, Fault> {
    if let Value::Number(position) = value
        && (0.0..=4096.0).contains(position)
        && position.fract() == 0.0
    {
        return Ok(*position as usize);
    }
    Err(Fault(
        "expected a scalar position within the input budget".into(),
    ))
}

fn displacement(value: &Value) -> Result<i64, Fault> {
    let Value::Number(position) = value else {
        return Err(Fault("expected an integer scroll displacement".into()));
    };
    if !position.is_finite() || position.fract() != 0.0 {
        return Err(Fault("expected an integer scroll displacement".into()));
    }
    if *position < i64::MIN as f64 || *position > i64::MAX as f64 {
        return Err(Fault("scroll displacement is out of range".into()));
    }
    Ok(*position as i64)
}
