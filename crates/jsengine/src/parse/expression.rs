use super::{Context, Expression, Kind, Parser, Rejection, reserved};
use crate::syntax::Item;

impl Parser {
    pub(super) fn expression(&mut self, minimum: u8) -> Result<Expression, Rejection> {
        self.enter()?;
        let start = self.tokens.peek().span.start;
        let result = self.binary(minimum);
        self.depth -= 1;
        result.map_err(|error| error.at(start..self.tokens.peek().span.start))
    }

    fn binary(&mut self, minimum: u8) -> Result<Expression, Rejection> {
        if minimum == 0
            && self.ahead()
            && !matches!(&self.tokens.look(1).kind, Kind::Name(name) if name == "function")
        {
            return self.arrow();
        }
        let mut left = self.prefix()?;
        while let Kind::Mark(operator) | Kind::Name(operator) = self.peek().clone() {
            let Some(precedence) = precedence(&operator) else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.take();
            let next = if precedence == 0 {
                precedence
            } else {
                precedence + 1
            };
            let right = self.expression(next)?;
            left = combine(left, operator, right)?;
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expression, Rejection> {
        if self.eat("typeof") {
            return Ok(Expression::Unary(
                "typeof".into(),
                Box::new(self.expression(8)?),
            ));
        }
        if self.eat("delete") {
            let target = self.expression(8)?;
            if matches!(target, Expression::Name(_) | Expression::Super(_)) {
                return Err(self.fault("strict delete target is forbidden"));
            }
            return Ok(Expression::Unary("delete".into(), Box::new(target)));
        }
        if self.eat("await") {
            if !self.asynchronous {
                return Err(self.fault("await requires an async function body"));
            }
            return Ok(Expression::Await(Box::new(self.expression(8)?)));
        }
        if self.at("++") || self.at("--") {
            let increment = self.eat("++");
            if !increment {
                self.expect("--")?;
            }
            let target = self.expression(8)?;
            return update(target, increment, true);
        }
        if self.at("!") || self.at("-") || self.at("+") {
            let Kind::Mark(operator) = self.take() else {
                unreachable!()
            };
            return Ok(Expression::Unary(operator, Box::new(self.expression(8)?)));
        }
        let value = self.left()?;
        let same = self.tokens.previous().map(|token| token.location.line)
            == Some(self.tokens.peek().location.line);
        if same && (self.at("++") || self.at("--")) {
            let increment = self.eat("++");
            if !increment {
                self.expect("--")?;
            }
            return update(value, increment, false);
        }
        Ok(value)
    }

    pub(super) fn left(&mut self) -> Result<Expression, Rejection> {
        let mut value = if self.eat("new") {
            self.construction()?
        } else {
            self.atom()?
        };
        loop {
            value = self.member(value)?;
            if !self.eat("(") {
                return Ok(value);
            }
            value = Expression::Call(Box::new(value), self.arguments()?);
        }
    }

    fn construction(&mut self) -> Result<Expression, Rejection> {
        if self.eat(".") {
            self.expect("target")?;
            if self.context == Context::Script {
                return Err(self.fault("new.target requires a function context"));
            }
            return Ok(Expression::Target);
        }
        let direct = self.at("super");
        let callee = self.atom()?;
        if direct && matches!(callee, Expression::Parent(_)) {
            return Err(self.fault("super() is not a new-expression callee"));
        }
        let callee = self.member(callee)?;
        self.expect("(")?;
        Ok(Expression::New(Box::new(callee), self.arguments()?))
    }

    fn member(&mut self, mut value: Expression) -> Result<Expression, Rejection> {
        loop {
            if self.eat(".") {
                if self.peek() == &Kind::End {
                    return Err(self.fault("expected property name"));
                }
                let Kind::Name(name) = self.take() else {
                    return Err(self.consumed("expected property name"));
                };
                value = Expression::Member(Box::new(value), Box::new(Expression::Text(name)));
            } else if self.eat("[") {
                let key = self.expression(0)?;
                self.expect("]")?;
                value = Expression::Member(Box::new(value), Box::new(key));
            } else {
                return Ok(value);
            }
        }
    }

    fn atom(&mut self) -> Result<Expression, Rejection> {
        let span = self.tokens.peek().span.clone();
        let location = self.tokens.peek().location;
        match self.take() {
            Kind::Number(value) => Ok(Expression::Number(value)),
            Kind::Text(value) => Ok(Expression::Text(value)),
            Kind::Name(name) => self.word(name),
            Kind::Mark(mark) if mark == "(" => {
                let value = self.expression(0)?;
                self.expect(")")?;
                Ok(value)
            }
            Kind::Mark(mark) if mark == "{" => self.fields().map(Expression::Object),
            Kind::Mark(mark) if mark == "[" => self.array(),
            _ => Err(Rejection::syntax(format!(
                "syntax at character {}: expected expression; unsupported syntax",
                location.character
            ))
            .at(span)),
        }
    }

    fn word(&mut self, name: String) -> Result<Expression, Rejection> {
        match name.as_str() {
            "true" => Ok(Expression::Boolean(true)),
            "false" => Ok(Expression::Boolean(false)),
            "null" => Ok(Expression::Null),
            "this" => Ok(Expression::This),
            "function" => self.function(Context::Function).map(Expression::Function),
            "async" => {
                if self.tokens.previous().map(|token| token.location.line)
                    != Some(self.tokens.peek().location.line)
                {
                    return Err(self.fault("async function requires the same line"));
                }
                self.expect("function")?;
                let previous = (self.context, self.asynchronous);
                self.context = Context::Function;
                self.asynchronous = false;
                let params = self.parameters()?;
                self.asynchronous = true;
                let body = self.activation(false)?;
                self.context = previous.0;
                self.asynchronous = previous.1;
                let mut function = super::function::finish(params, body)?;
                function.asynchronous = true;
                Ok(Expression::Function(function))
            }
            "super" => self.superclass(),
            "arguments" if self.context == Context::Field => {
                Err(self.fault("arguments is forbidden in a field initializer"))
            }
            "class" => self.class().map(Expression::Class),
            _ if reserved(&name) => Err(self.consumed(&format!("unsupported keyword: {name}"))),
            _ => Ok(Expression::Name(name)),
        }
    }

    fn array(&mut self) -> Result<Expression, Rejection> {
        let mut items = Vec::new();
        while !self.eat("]") {
            if self.eat(",") {
                items.push(None);
                continue;
            }
            items.push(Some(self.item()?));
            if self.eat("]") {
                break;
            }
            self.expect(",")?;
        }
        Ok(Expression::Array(items))
    }

    fn item(&mut self) -> Result<Item, Rejection> {
        let spread = self.eat("...");
        let value = self.expression(0)?;
        Ok(if spread {
            Item::Spread(value)
        } else {
            Item::Value(value)
        })
    }

    pub(super) fn arguments(&mut self) -> Result<Vec<Item>, Rejection> {
        let mut arguments = Vec::new();
        while !self.eat(")") {
            arguments.push(self.item()?);
            if self.eat(")") {
                break;
            }
            self.expect(",")?;
        }
        Ok(arguments)
    }
}

fn precedence(operator: &str) -> Option<u8> {
    match operator {
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&&=" | "||=" | "??=" => Some(0),
        "||" => Some(1),
        "&&" => Some(2),
        "===" | "!==" => Some(3),
        "<" | "<=" | ">" | ">=" | "instanceof" | "in" => Some(4),
        "+" | "-" => Some(5),
        "*" | "/" | "%" => Some(6),
        _ => None,
    }
}

fn combine(left: Expression, operator: String, right: Expression) -> Result<Expression, Rejection> {
    if precedence(&operator) != Some(0) {
        return Ok(Expression::Binary(
            operator,
            Box::new(left),
            Box::new(right),
        ));
    }
    target(&left)?;
    Ok(Expression::Assign(crate::syntax::Assignment {
        operator,
        target: Box::new(left),
        value: Box::new(right),
    }))
}

fn target(expression: &Expression) -> Result<(), Rejection> {
    match expression {
        Expression::Name(name) if matches!(name.as_str(), "eval" | "arguments") => {
            Err(Rejection::syntax("strict assignment target is forbidden"))
        }
        Expression::Name(_) | Expression::Member(..) | Expression::Super(_) => Ok(()),
        _ => Err(Rejection::syntax(
            "assignment target must be a variable or property",
        )),
    }
}

fn update(expression: Expression, increment: bool, prefix: bool) -> Result<Expression, Rejection> {
    target(&expression)?;
    Ok(Expression::Update(crate::syntax::Update {
        target: Box::new(expression),
        delta: if increment { 1 } else { -1 },
        prefix,
    }))
}
