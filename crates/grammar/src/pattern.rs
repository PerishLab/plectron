pub(crate) enum Pattern {
    Literal(String),
    Class(Vec<Item>, bool),
    Any,
    Name(String),
    Sequence(Vec<Self>),
    Choice(Vec<Self>),
    Repeat(Box<Self>, usize, Option<usize>),
    Look(Box<Self>, bool),
}

pub(crate) enum Item {
    Range(char, char),
    Space,
}

impl Item {
    pub fn accepts(&self, value: char) -> bool {
        match self {
            Self::Range(start, end) => (*start..=*end).contains(&value),
            Self::Space => value.is_whitespace(),
        }
    }
}
