use super::Content;
use crate::{Fault, addons};

impl Content {
    pub(crate) fn scrolling(&mut self, changes: Vec<crate::display::Change>) -> Result<(), Fault> {
        self.check()?;
        let runtime = self.runtime()?;
        for change in changes {
            if !change.notify {
                continue;
            }
            let Some(node) = change.node else {
                continue;
            };
            if runtime
                .addons()
                .document
                .connected(node)
                .map_err(crate::error)?
            {
                runtime
                    .host(|engine, web| addons::event::scroll(engine, web, change).map(|_| ()))?;
            }
        }
        Ok(())
    }

    pub fn evaluate(&mut self, source: &str) -> Result<String, Fault> {
        self.check()?;
        let runtime = self.runtime()?;
        runtime
            .execute(source)
            .map(|value| value.text())
            .map_err(|failure| runtime.diagnostic(&failure))
    }
    pub fn input(
        &mut self,
        input: &crate::keyboard::Input,
    ) -> Result<crate::keyboard::Outcome, Fault> {
        self.check()?;
        addons::input(self.runtime()?, input)
    }
    pub fn edit(&mut self, edit: &crate::Edit) -> Result<(), Fault> {
        self.check()?;
        addons::edit(self.runtime()?, edit)
    }
    pub fn compose(&mut self, input: &crate::ime::Input) -> Result<bool, Fault> {
        self.check()?;
        addons::compose(self.runtime()?, input)
    }
    pub fn receive(&mut self, message: String) -> Result<(), Fault> {
        self.check()?;
        if message.len() > 16_384 {
            return Err(Fault("message budget exhausted".into()));
        }
        addons::message(self.runtime()?, message)
    }
    pub fn pending(&mut self) -> bool {
        self.runtime
            .as_mut()
            .is_some_and(|runtime| runtime.addons().pending())
    }

    pub fn poll(&mut self) -> Result<bool, Fault> {
        self.check()?;
        let runtime = self.runtime()?;
        let count = runtime.host(|engine, addons| addons.poll(engine))?;
        if count == 0 {
            return Ok(false);
        }
        runtime.checkpoint()?;
        self.render()?;
        Ok(true)
    }

    pub fn messages(&mut self) -> Vec<String> {
        self.runtime
            .as_mut()
            .map(|runtime| runtime.addons().messages())
            .unwrap_or_default()
    }
}
