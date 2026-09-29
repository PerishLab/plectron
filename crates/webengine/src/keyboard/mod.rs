mod defaults;
use crate::{Edit, Fault, error};
pub use shared::shortcut::{Binding, Chord, Handle, Input, Modifiers, Outcome, Scope, Set, Stroke};

#[derive(Debug, Clone, Copy)]
pub enum Context {
    Editor,
    Page,
}
impl Context {
    pub fn parse(name: &str) -> Result<Self, Fault> {
        match name {
            "editor" => Ok(Self::Editor),
            "page" => Ok(Self::Page),
            _ => Err(Fault("unknown shortcut scope".into())),
        }
    }
}

pub(crate) struct Keyboard {
    editor: Scope<Edit>,
    page: Scope<Edit>,
}
impl Default for Keyboard {
    fn default() -> Self {
        let mut keyboard = Self {
            editor: Scope::default(),
            page: Scope::default(),
        };
        keyboard
            .editor
            .register(defaults::editor())
            .expect("valid editor bindings");
        keyboard
            .page
            .register(defaults::page())
            .expect("valid page bindings");
        keyboard
    }
}
impl Keyboard {
    pub fn scope(&mut self, context: Context) -> &mut Scope<Edit> {
        match context {
            Context::Editor => &mut self.editor,
            Context::Page => &mut self.page,
        }
    }

    pub fn candidates(&self, stroke: &Stroke, editing: bool) -> Vec<Binding<Edit>> {
        let mut bindings = if editing {
            self.editor.candidates(stroke)
        } else {
            Vec::new()
        };
        bindings.extend(self.page.candidates(stroke));
        bindings
    }
}

pub fn command(name: &str) -> Result<Edit, Fault> {
    use crate::Direction;
    match name {
        "backward" => Ok(Edit::Backward),
        "forward" => Ok(Edit::Forward),
        "selectAll" => Ok(Edit::All),
        "commit" => Ok(Edit::Commit),
        "activate" => Ok(Edit::Activate),
        "blur" => Ok(Edit::Blur),
        "next" => Ok(Edit::Next),
        _ => {
            let (name, extend) = name
                .strip_prefix("extend:")
                .map_or((name, false), |name| (name, true));
            let direction = match name {
                "left" => Direction::Left,
                "right" => Direction::Right,
                "start" => Direction::Start,
                "end" => Direction::End,
                _ => return Err(Fault("unknown editing command".into())),
            };
            Ok(Edit::Move { direction, extend })
        }
    }
}

pub(crate) fn validate(input: &Input) -> Result<(), Fault> {
    input.validate().map_err(error)
}
