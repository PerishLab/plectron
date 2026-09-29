use super::{Parser, Rejection, Statement};
use crate::syntax::{Catch, Handler};

impl Parser {
    pub(super) fn throwing(&mut self) -> Result<Statement, Rejection> {
        if self.tokens.previous().map(|token| token.location.line)
            != Some(self.tokens.peek().location.line)
        {
            return Err(self.fault("line terminator after throw"));
        }
        let value = self.expression(0)?;
        self.expect(";")?;
        Ok(Statement::Throw(value))
    }

    pub(super) fn handler(&mut self) -> Result<Statement, Rejection> {
        self.expect("{")?;
        let body = self.body()?;
        let catch = if self.eat("catch") {
            Some(self.catching()?)
        } else {
            None
        };
        let finally = if self.eat("finally") {
            self.expect("{")?;
            Some(self.body()?)
        } else {
            None
        };
        if catch.is_none() && finally.is_none() {
            return Err(self.fault("try requires catch or finally"));
        }
        Ok(Statement::Try(Handler {
            body,
            catch,
            finally,
        }))
    }

    fn catching(&mut self) -> Result<Catch, Rejection> {
        let name = if self.eat("(") {
            let name = self.name()?;
            self.expect(")")?;
            Some(name)
        } else {
            None
        };
        self.expect("{")?;
        let body = self.body()?;
        super::validate(&body, &name.iter().cloned().collect::<Vec<_>>())?;
        Ok(Catch { name, body })
    }
}
