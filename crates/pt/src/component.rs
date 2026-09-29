mod plain;
mod syntax;

pub use plain::compile;

use crate::Fault;
use std::collections::BTreeSet;
use syntax::{fields, intrinsic, literal, word};

struct Parser<'a> {
    source: &'a str,
    stack: Vec<(String, String)>,
    code: String,
    listeners: String,
    count: usize,
    refs: BTreeSet<String>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            stack: Vec::new(),
            code: String::new(),
            listeners: String::new(),
            count: 0,
            refs: BTreeSet::new(),
        }
    }

    fn parse(&mut self) -> Result<(), Fault> {
        while !self.source.is_empty() {
            if !self.source.starts_with('<') {
                self.text()?;
            } else {
                self.element()?;
            }
        }
        if self.stack.len() != 1 {
            return Err(Fault("unclosed template element".into()));
        }
        Ok(())
    }

    fn text(&mut self) -> Result<(), Fault> {
        let end = self.source.find('<').unwrap_or(self.source.len());
        let value = &self.source[..end];
        if value.contains(['{', '}']) {
            return Err(Fault("template expressions are not supported".into()));
        }
        if !value.trim().is_empty() {
            let parent = &self.stack.last().expect("template host").1;
            let variable = format!("part{}", self.count);
            self.count += 1;
            self.code.push_str(&format!(
                "const {variable} = document.createTextNode({value:?});\n{parent}.appendChild({variable});\n"
            ));
        }
        self.source = &self.source[end..];
        Ok(())
    }

    fn element(&mut self) -> Result<(), Fault> {
        let (token, rest) = self.source[1..]
            .split_once('>')
            .ok_or_else(|| Fault("unterminated template element".into()))?;
        self.source = rest;
        if let Some(name) = token.strip_prefix('/') {
            let (open, _) = self
                .stack
                .pop()
                .ok_or_else(|| Fault("unexpected close".into()))?;
            if open != name || self.stack.is_empty() {
                return Err(Fault("mismatched closing element".into()));
            }
            return Ok(());
        }
        let closed = token.ends_with('/');
        let token = token.trim_end_matches('/').trim();
        let (name, rest) = token.split_once(char::is_whitespace).unwrap_or((token, ""));
        if !word(name) {
            return Err(Fault("tag must be a single word".into()));
        }
        let attrs = fields(rest)?;
        let variable = format!("part{}", self.count);
        self.count += 1;
        if name == "text" {
            if !closed || attrs.iter().any(|(key, _)| key != "ref") {
                return Err(Fault(
                    "text accepts only ref and must be self closing".into(),
                ));
            }
            self.code.push_str(&format!(
                "const {variable} = document.createTextNode(\"\");\n"
            ));
        } else if intrinsic(name) {
            if name == "img" && !closed {
                return Err(Fault("img must be self closing".into()));
            }
            self.code.push_str(&format!(
                "const {variable} = document.createElement({name:?});\n"
            ));
        } else {
            return Err(Fault("unknown intrinsic tag".into()));
        }
        for (key, value) in attrs {
            if key == "ref" {
                if !word(&value) {
                    return Err(Fault("ref must be a single word".into()));
                }
                if !self.refs.insert(value.clone()) {
                    return Err(Fault(format!("duplicate ref: {value}")));
                }
                self.code.push_str(&format!("refs.{value} = {variable};\n"));
            } else if literal(&key) {
                self.code
                    .push_str(&format!("{variable}.setAttribute({key:?}, {value:?});\n"));
            } else if let Some(event) = key.strip_prefix('@') {
                self.listeners.push_str(&format!(
                    "{variable}.addEventListener({event:?}, handlers.{value}, {{scope: scope}});\n"
                ));
            } else {
                return Err(Fault(format!("unsupported template attribute: {key}")));
            }
        }
        let parent = &self.stack.last().expect("template host").1;
        self.code
            .push_str(&format!("{parent}.appendChild({variable});\n"));
        if !closed {
            if name == "text" {
                return Err(Fault("text tags must be self closing".into()));
            }
            self.stack.push((name.into(), variable));
        }
        Ok(())
    }
}
