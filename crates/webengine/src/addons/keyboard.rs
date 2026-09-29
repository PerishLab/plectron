use super::{Web, editing};
use crate::{
    Edit, Fault,
    keyboard::{Input, Outcome},
};
use jsruntime::Runtime;

pub fn input(runtime: &mut Runtime<Web>, input: &Input) -> Result<Outcome, Fault> {
    crate::keyboard::validate(input)?;
    let (outcome, text) = match input {
        Input::Text(text) => (Outcome::default(), text),
        Input::Key(stroke) => {
            let web = runtime.addons();
            let editing = web
                .document
                .focused()
                .is_some_and(|node| web.document.editable(node));
            let candidates = web.keyboard.candidates(stroke, editing);
            let outcome =
                shared::shortcut::dispatch(candidates, |command| execute(runtime, &command))?;
            (outcome, &stroke.text)
        }
    };
    let editing = runtime
        .addons()
        .document
        .focused()
        .is_some_and(|node| runtime.addons().document.editable(node));
    if !outcome.consumed && editing && !text.is_empty() {
        editing::edit(runtime, &Edit::Insert(text.clone()))?;
    }
    Ok(outcome)
}

fn execute(runtime: &mut Runtime<Web>, command: &Edit) -> Result<bool, Fault> {
    let document = &runtime.addons().document;
    let applicable = match command {
        Edit::Next => document
            .walk()
            .into_iter()
            .any(|node| document.focusable(node)),
        Edit::Activate => document
            .focused()
            .is_some_and(|node| document.activatable(node)),
        _ => document.focused().is_some(),
    };
    if !applicable {
        return Ok(false);
    }
    editing::edit(runtime, command)?;
    Ok(true)
}
