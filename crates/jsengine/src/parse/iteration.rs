use super::{Parser, Rejection, Statement};
use crate::syntax::Iteration;

impl Parser {
    pub(super) fn iteration(&mut self) -> Result<Statement, Rejection> {
        self.expect("(")?;
        let mutable = if self.eat("let") {
            true
        } else {
            self.expect("const")?;
            false
        };
        let name = self.name()?;
        self.expect("of")?;
        let source = self.expression(0)?;
        self.expect(")")?;
        self.loops += 1;
        let body = self.statement();
        self.loops -= 1;
        let body = Box::new(body?);
        if matches!(body.as_ref(), Statement::Declare { .. }) {
            return Err(self.fault("lexical declaration needs a block"));
        }
        Ok(Statement::For(Iteration {
            name,
            mutable,
            source,
            body,
        }))
    }

    pub(super) fn control(&mut self) -> Result<Statement, Rejection> {
        if self.loops == 0 {
            return Err(self.fault("loop control outside loop"));
        }
        let statement = if self.eat("break") {
            Statement::Break
        } else {
            self.expect("continue")?;
            Statement::Continue
        };
        self.expect(";")?;
        Ok(statement)
    }
}
