use super::scan::{self, Kind, Token};
use super::syntax::{Expression, Statement};
use crate::Rejection;
use grammar::Stream;

mod class;
mod context;
use context::Context;
mod arrow;
mod expression;
mod function;
mod handler;
mod iteration;
mod object;
mod statement;

pub(crate) fn program(
    source: &str,
    origin: Option<&crate::Source>,
) -> Result<Vec<Statement>, Rejection> {
    let mut parser = Parser {
        tokens: scan::tokens(source)?,
        depth: 0,
        functions: 0,
        asynchronous: false,
        loops: 0,
        context: Context::Script,
        source: origin.cloned(),
    };
    let mut body = Vec::new();
    while parser.peek() != &Kind::End {
        body.push(parser.statement()?);
    }
    validate(&body, &[])?;
    Ok(body)
}

struct Parser {
    source: Option<crate::Source>,
    tokens: Stream<Token>,
    depth: usize,
    functions: usize,
    asynchronous: bool,
    loops: usize,
    context: Context,
}

impl Parser {
    fn peek(&self) -> &Kind {
        &self.tokens.peek().kind
    }

    fn take(&mut self) -> Kind {
        self.tokens.take().kind.clone()
    }

    fn fault(&self, note: &str) -> Rejection {
        Rejection::syntax(format!(
            "syntax at character {}: {note}",
            self.tokens.peek().location.character
        ))
        .at(self.tokens.peek().span.clone())
    }

    fn consumed(&self, note: &str) -> Rejection {
        let token = self.tokens.previous().unwrap_or_else(|| self.tokens.peek());
        Rejection::syntax(format!(
            "syntax at character {}: {note}",
            token.location.character
        ))
        .at(token.span.clone())
    }

    fn at(&self, text: &str) -> bool {
        matches!(self.peek(), Kind::Name(value) | Kind::Mark(value) if value == text)
    }

    fn eat(&mut self, text: &str) -> bool {
        self.tokens.eat(
            |token| matches!(&token.kind, Kind::Name(value) | Kind::Mark(value) if value == text),
        )
    }

    fn expect(&mut self, text: &str) -> Result<(), Rejection> {
        if self.eat(text) {
            Ok(())
        } else {
            Err(self.fault(&format!("expected {text}")))
        }
    }

    fn name(&mut self) -> Result<String, Rejection> {
        match self.take() {
            Kind::Name(name)
                if !reserved(&name) && !matches!(name.as_str(), "eval" | "arguments") =>
            {
                Ok(name)
            }
            Kind::End => Err(self.fault("expected supported identifier")),
            _ => Err(self.consumed("expected supported identifier")),
        }
    }

    fn enter(&mut self) -> Result<(), Rejection> {
        if self.depth >= 64 {
            return Err(crate::Fault("syntax nesting budget exhausted".into()).into());
        }
        self.depth += 1;
        Ok(())
    }
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "let"
            | "const"
            | "var"
            | "function"
            | "if"
            | "else"
            | "return"
            | "true"
            | "false"
            | "null"
            | "new"
            | "this"
            | "class"
            | "while"
            | "for"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "throw"
            | "try"
            | "catch"
            | "finally"
            | "import"
            | "export"
            | "default"
            | "async"
            | "await"
            | "yield"
            | "delete"
            | "typeof"
            | "void"
            | "in"
            | "instanceof"
            | "with"
            | "debugger"
            | "extends"
            | "super"
            | "enum"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
    )
}

fn validate(body: &[Statement], params: &[String]) -> Result<(), Rejection> {
    let mut names: std::collections::BTreeSet<_> = params.iter().collect();
    for statement in body {
        match statement.plain() {
            Statement::Declare { name, .. } if !names.insert(name) => {
                return Err(Rejection::syntax(format!("duplicate binding: {name}")));
            }
            _ => (),
        }
    }
    Ok(())
}
