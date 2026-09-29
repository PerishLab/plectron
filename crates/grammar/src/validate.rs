use crate::{Error, Grammar, Pattern};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn check(grammar: &Grammar) -> Result<BTreeSet<String>, Error> {
    let mut names = BTreeSet::new();
    for (name, pattern) in &grammar.rules {
        if !names.insert(name.clone()) {
            return Err(Error::grammar(format!("duplicate rule: {name}")));
        }
        pattern.references(grammar)?;
    }
    let mut empty = BTreeSet::new();
    loop {
        let before = empty.len();
        for (name, pattern) in &grammar.rules {
            if pattern.nullable(&empty) {
                empty.insert(name.clone());
            }
        }
        if before == empty.len() {
            break;
        }
    }
    let mut graph = BTreeMap::new();
    for (name, pattern) in &grammar.rules {
        pattern.repetitions(&empty)?;
        let mut edges = BTreeSet::new();
        pattern.leading(&empty, &mut edges);
        graph.insert(name.clone(), edges);
    }
    let mut done = BTreeSet::new();
    for name in graph.keys() {
        cycle(name, &graph, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(empty)
}

impl Pattern {
    fn children(&self) -> &[Pattern] {
        match self {
            Pattern::Sequence(parts) | Pattern::Choice(parts) => parts,
            Pattern::Repeat(inner, ..) | Pattern::Look(inner, _) => std::slice::from_ref(inner),
            _ => &[],
        }
    }

    fn references(&self, grammar: &Grammar) -> Result<(), Error> {
        if let Pattern::Name(name) = self {
            grammar.rule(name)?;
        }
        for child in self.children() {
            child.references(grammar)?;
        }
        Ok(())
    }

    fn nullable(&self, empty: &BTreeSet<String>) -> bool {
        match self {
            Pattern::Literal(text) => text.is_empty(),
            Pattern::Name(name) => empty.contains(name),
            Pattern::Sequence(parts) => parts.iter().all(|part| part.nullable(empty)),
            Pattern::Choice(parts) => parts.iter().any(|part| part.nullable(empty)),
            Pattern::Repeat(inner, minimum, _) => *minimum == 0 || inner.nullable(empty),
            Pattern::Look(..) => true,
            _ => false,
        }
    }

    fn repetitions(&self, empty: &BTreeSet<String>) -> Result<(), Error> {
        if let Pattern::Repeat(inner, _, None) = self
            && inner.nullable(empty)
        {
            return Err(Error::grammar("nullable repetition"));
        }
        for child in self.children() {
            child.repetitions(empty)?;
        }
        Ok(())
    }

    fn leading(&self, empty: &BTreeSet<String>, names: &mut BTreeSet<String>) {
        if let Pattern::Name(name) = self {
            names.insert(name.clone());
        }
        for child in self.children() {
            child.leading(empty, names);
            if matches!(self, Pattern::Sequence(_)) && !child.nullable(empty) {
                break;
            }
        }
    }
}

fn cycle(
    name: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    path: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> Result<(), Error> {
    if done.contains(name) {
        return Ok(());
    }
    if !path.insert(name.into()) {
        return Err(Error::grammar(format!("left recursion: {name}")));
    }
    for next in &graph[name] {
        cycle(next, graph, path, done)?;
    }
    path.remove(name);
    done.insert(name.into());
    Ok(())
}
