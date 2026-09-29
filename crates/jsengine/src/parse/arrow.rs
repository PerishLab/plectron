use super::{Expression, Kind, Parser, Rejection};
use crate::syntax::{Parameter, Parameters};

impl Parser {
    pub(super) fn ahead(&self) -> bool {
        if self.at("async") {
            return true;
        }
        if matches!(self.peek(), Kind::Name(_)) {
            return self.mark(1, "=>");
        }
        if !self.at("(") {
            return false;
        }
        let mut offset = 0;
        let mut depth = 0;
        loop {
            if self.tokens.look(offset).kind == Kind::End {
                return false;
            }
            if self.mark(offset, "(") {
                depth += 1;
            } else if self.mark(offset, ")") {
                depth -= 1;
            }
            if depth == 0 {
                return self.mark(offset + 1, "=>");
            }
            offset += 1;
        }
    }

    fn mark(&self, offset: usize, mark: &str) -> bool {
        matches!(&self.tokens.look(offset).kind, Kind::Mark(value) if value == mark)
    }

    pub(super) fn arrow(&mut self) -> Result<Expression, Rejection> {
        let asynchronous = self.eat("async");
        if asynchronous
            && self.tokens.previous().map(|token| token.location.line)
                != Some(self.tokens.peek().location.line)
        {
            return Err(self.fault("async arrow requires the same line"));
        }
        let previous = self.asynchronous;
        self.asynchronous = false;
        let params = if self.at("(") {
            self.parameters()?
        } else {
            Parameters {
                items: vec![Parameter {
                    name: self.name()?,
                    initializer: None,
                }],
                rest: None,
            }
        };
        if self.tokens.previous().map(|token| token.location.line)
            != Some(self.tokens.peek().location.line)
        {
            return Err(self.fault("arrow must follow parameters on the same line"));
        }
        self.expect("=>")?;
        self.asynchronous = asynchronous;
        let body = self.activation(true)?;
        self.asynchronous = previous;
        let mut function = super::function::finish(params, body)?;
        function.asynchronous = asynchronous;
        Ok(Expression::Arrow(function))
    }
}
