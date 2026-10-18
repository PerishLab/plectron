use super::super::{Web, event};
use crate::{Fault, error, ime};
use jsruntime::Runtime;

pub fn input(runtime: &mut Runtime<Web>, input: &ime::Input) -> Result<bool, Fault> {
    input.validate().map_err(error)?;
    let id = input.id();
    let node = match runtime.addons().document.focused() {
        Some(node) if runtime.addons().document.editable(node) => node,
        _ => return Ok(false),
    };
    let active = runtime.addons().composition;
    match input {
        ime::Input::Update(_) | ime::Input::Mark { .. } => match active {
            None => (),
            Some(session) if session == (id, node) => (),
            Some(_) => return Ok(false),
        },
        ime::Input::Commit { .. } | ime::Input::Cancel { .. } if active != Some((id, node)) => {
            return Ok(false);
        }
        _ => (),
    }
    let effect = runtime
        .addons()
        .document
        .compose(node, input)
        .map_err(error)?;
    match effect {
        ime::Effect::Ignored => Ok(false),
        ime::Effect::Updated => {
            if active.is_none() {
                runtime.addons().composition = Some((id, node));
            }
            Ok(true)
        }
        ime::Effect::Cancelled => {
            runtime.addons().composition = None;
            Ok(true)
        }
        ime::Effect::Committed { changed: false } => {
            runtime.addons().composition = None;
            Ok(true)
        }
        ime::Effect::Committed { changed: true } => {
            runtime.addons().composition = None;
            let ime::Input::Commit { text, .. } = input else {
                unreachable!()
            };
            runtime.host(|engine, web| event::composition(engine, web, node, text))?;
            Ok(true)
        }
    }
}

pub(in crate::addons) fn abandon(web: &mut Web) {
    web.composition = None;
}
