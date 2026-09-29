#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub character: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy)]
pub struct Source<'a> {
    text: &'a str,
}

impl<'a> Source<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn slice(&self, span: Span) -> Option<&'a str> {
        self.text.get(span.start..span.end)
    }

    pub fn location(&self, offset: usize) -> Option<Location> {
        let prefix = self.text.get(..offset)?;
        let character = prefix.chars().count();
        let line = prefix.bytes().filter(|c| *c == b'\n').count();
        let column = prefix.rsplit('\n').next()?.chars().count();
        Some(Location {
            character,
            line,
            column,
        })
    }
}

impl Location {
    pub(crate) fn advance(&mut self, text: &str) {
        for value in text.chars() {
            self.character += 1;
            if value == '\n' {
                self.line += 1;
                self.column = 0;
            } else {
                self.column += 1;
            }
        }
    }
}
