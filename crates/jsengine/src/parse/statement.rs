use super::{Parser, Rejection, Statement};

impl Parser {
    pub(super) fn statement(&mut self) -> Result<Statement, Rejection> {
        self.enter()?;
        let start = self.tokens.peek().span.start;
        let result = self.branch();
        self.depth -= 1;
        result.map(|statement| {
            let end = self.tokens.previous().map_or(start, |token| token.span.end);
            self.located(statement, start..end)
        })
    }

    pub(super) fn located(&self, statement: Statement, span: std::ops::Range<usize>) -> Statement {
        match &self.source {
            Some(source) => Statement::Located(Box::new(statement), source.site(span)),
            None => statement,
        }
    }

    fn branch(&mut self) -> Result<Statement, Rejection> {
        if self.eat(";") {
            return Ok(Statement::Empty);
        }
        if self.eat("try") {
            return self.handler();
        }
        if self.eat("throw") {
            return self.throwing();
        }
        if self.eat("class") {
            return self.declaration();
        }
        if self.eat("let") {
            return self.declare(true);
        }
        if self.eat("const") {
            return self.declare(false);
        }
        if self.eat("{") {
            return self.body().map(Statement::Block);
        }
        if self.eat("if") {
            return self.condition();
        }
        if self.eat("for") {
            return self.iteration();
        }
        if self.at("break") || self.at("continue") {
            return self.control();
        }
        if self.eat("return") {
            return self.returning();
        }
        let value = self.expression(0)?;
        self.expect(";")?;
        Ok(Statement::Evaluate(value))
    }

    fn declare(&mut self, mutable: bool) -> Result<Statement, Rejection> {
        let name = self.name()?;
        self.expect("=")?;
        let value = self.expression(0)?;
        self.expect(";")?;
        Ok(Statement::Declare {
            name,
            mutable,
            value,
        })
    }

    pub(super) fn body(&mut self) -> Result<Vec<Statement>, Rejection> {
        let mut statements = Vec::new();
        while !self.eat("}") {
            statements.push(self.statement()?);
        }
        super::validate(&statements, &[])?;
        Ok(statements)
    }

    fn condition(&mut self) -> Result<Statement, Rejection> {
        self.expect("(")?;
        let condition = self.expression(0)?;
        self.expect(")")?;
        let yes = Box::new(self.statement()?);
        let no = if self.eat("else") {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        if matches!(yes.plain(), Statement::Declare { .. }) {
            return Err(self.fault("lexical declaration needs a block"));
        }
        if matches!(
            no.as_deref().map(Statement::plain),
            Some(Statement::Declare { .. })
        ) {
            return Err(self.fault("lexical declaration needs a block"));
        }
        Ok(Statement::If { condition, yes, no })
    }

    fn returning(&mut self) -> Result<Statement, Rejection> {
        if self.functions == 0 {
            return Err(self.fault("return outside function"));
        }
        if self.tokens.previous().map(|token| token.location.line)
            != Some(self.tokens.peek().location.line)
        {
            return Err(
                self.fault("return requires an explicit semicolon or expression on the same line")
            );
        }
        let value = if self.at(";") {
            None
        } else {
            Some(self.expression(0)?)
        };
        self.expect(";")?;
        Ok(Statement::Return(value))
    }
}
