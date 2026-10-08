use super::{Expression, Kind, Parser, Rejection};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    Script,
    Function,
    Method,
    Derived,
    Field,
}

impl Parser {
    pub(super) fn superclass(&mut self) -> Result<Expression, Rejection> {
        if self.eat("(") {
            if self.context != Context::Derived {
                return Err(self.fault("super() requires a derived constructor"));
            }
            return self.arguments().map(Expression::Parent);
        }
        if matches!(self.context, Context::Script | Context::Function) {
            return Err(self.fault("super property requires a method context"));
        }
        let key = if self.eat(".") {
            let Kind::Name(name) = self.take() else {
                return Err(self.fault("expected property name"));
            };
            Expression::Text(name)
        } else {
            self.expect("[")?;
            let key = self.expression(0)?;
            self.expect("]")?;
            key
        };
        Ok(Expression::Super(Box::new(key)))
    }

    pub(super) fn initializer(&mut self) -> Result<Option<Expression>, Rejection> {
        let previous = self.context;
        self.context = Context::Field;
        let value = if self.eat("=") {
            self.initializing().map(Some)
        } else {
            Ok(None)
        };
        self.context = previous;
        let value = value?;
        self.expect(";")?;
        Ok(value)
    }

    pub(super) fn initializing(&mut self) -> Result<Expression, Rejection> {
        let start = self.tokens.peek().span.start;
        let expression = self.expression(0)?;
        let end = self.tokens.previous().map_or(start, |token| token.span.end);
        Ok(match &self.source {
            Some(source) => Expression::Located(Box::new(expression), source.site(start..end)),
            None => expression,
        })
    }
}
