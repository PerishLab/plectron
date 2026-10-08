use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Expression {
    Located(Box<Expression>, crate::Site),
    Await(Box<Expression>),
    Number(f64),
    Text(String),
    Boolean(bool),
    Null,
    This,
    Target,
    Super(Box<Expression>),
    Parent(Vec<Item>),
    Name(String),
    Object(Vec<(Key, Body)>),
    Array(Vec<Option<Item>>),
    Function(Function),
    Arrow(Function),
    Class(Class),
    Member(Box<Expression>, Box<Expression>),
    Call(Box<Expression>, Vec<Item>),
    New(Box<Expression>, Vec<Item>),
    Unary(String, Box<Expression>),
    Binary(String, Box<Expression>, Box<Expression>),
    Assign(Assignment),
    Update(Update),
}

#[derive(Debug, Clone)]
pub enum Item {
    Value(Expression),
    Spread(Expression),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Located(Box<Statement>, crate::Site),
    Declare {
        name: String,
        mutable: bool,
        value: Expression,
    },
    Evaluate(Expression),
    Block(Vec<Statement>),
    If {
        condition: Expression,
        yes: Box<Statement>,
        no: Option<Box<Statement>>,
    },
    Return(Option<Expression>),
    Throw(Expression),
    Try(Handler),
    Empty,
    For(Iteration),
    Break,
    Continue,
}

impl Statement {
    pub fn plain(&self) -> &Self {
        match self {
            Self::Located(statement, _) => statement.plain(),
            statement => statement,
        }
    }

    pub fn site(&self) -> Option<crate::Site> {
        match self {
            Self::Located(_, site) => Some(site.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Iteration {
    pub name: String,
    pub mutable: bool,
    pub source: Expression,
    pub body: Box<Statement>,
}

#[derive(Debug, Clone)]
pub struct Handler {
    pub body: Vec<Statement>,
    pub catch: Option<Catch>,
    pub finally: Option<Vec<Statement>>,
}

#[derive(Debug, Clone)]
pub struct Catch {
    pub name: Option<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, Default)]
pub struct Function {
    pub asynchronous: bool,
    pub params: Arc<Parameters>,
    pub body: Arc<[Statement]>,
}

#[derive(Debug, Clone, Default)]
pub struct Parameters {
    pub items: Vec<Parameter>,
    pub rest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub initializer: Option<Expression>,
}

impl Parameters {
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.items.iter().map(|item| &item.name).chain(&self.rest)
    }

    pub fn arity(&self) -> usize {
        self.items
            .iter()
            .take_while(|item| item.initializer.is_none())
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: Option<String>,
    pub heritage: Option<Box<Expression>>,
    pub constructor: Option<Function>,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone)]
pub enum Key {
    Literal(String),
    Computed(Expression),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub key: Key,
    pub placement: Placement,
    pub body: Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Instance,
    Static,
}

#[derive(Debug, Clone)]
pub enum Body {
    Method(Function),
    Get(Function),
    Set(Function),
    Field(Option<Expression>),
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub operator: String,
    pub target: Box<Expression>,
    pub value: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct Update {
    pub target: Box<Expression>,
    pub delta: i8,
    pub prefix: bool,
}
