mod token;
use crate::{Document, Fault, Node};
use token::{Scanner, Token};

pub fn parse(source: &str) -> Result<Document, Fault> {
    if source.len() > 65_536 {
        return Err(Fault("HTML budget exhausted".into()));
    }
    let mut scanner = Scanner::new(source)?;
    let mut document = Document::default();
    let mut stack = vec![("document".to_string(), document.root())];
    while let Some(token) = scanner.next()? {
        accept(&mut document, &mut stack, token).map_err(|error| scanner.context(error))?;
    }
    if stack.len() != 1 {
        return Err(scanner.context(Fault("unclosed HTML element".into())));
    }
    Ok(document)
}

fn accept(
    document: &mut Document,
    stack: &mut Vec<(String, Node)>,
    token: Token,
) -> Result<(), Fault> {
    let parent = stack.last().expect("root exists").1;
    match token {
        Token::Open { name, attributes } => {
            let node = document.element(&name)?;
            for (key, value) in attributes {
                document.attribute(node, &key, &value)?;
            }
            document.append(parent, node)?;
            if !matches!(name.as_str(), "link" | "input" | "img") {
                stack.push((name, node));
            }
        }
        Token::Close(name) => {
            if stack.len() == 1 || stack.last().expect("root exists").0 != name {
                return Err(Fault(format!("mismatched closing tag: {name}")));
            }
            stack.pop();
        }
        Token::Text(text) => {
            if !text.is_empty() {
                let node = document.text(&text)?;
                document.append(parent, node)?;
            }
        }
    }
    Ok(())
}
