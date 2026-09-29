use crate::Result;
use domengine::{Constraints, Document, Extent, Node, html};

pub(super) struct Workload {
    pub document: Document,
    scenario: String,
    root: Node,
    text: Node,
    width: usize,
}

impl Workload {
    pub fn new(scenario: &str) -> Result<Self> {
        if !matches!(
            scenario,
            "tiny" | "paint" | "text" | "constraints" | "flex" | "large" | "nested"
        ) {
            return Err(format!("unknown layout scenario: {scenario}"));
        }
        let style = if matches!(scenario, "flex" | "nested") {
            "display: flex; width: 1200px;"
        } else {
            ""
        };
        let count = match scenario {
            "tiny" => 1,
            "nested" => 8,
            "large" => 140,
            _ => 48,
        };
        let mut children = "<p style='flex-grow: 1;'>ABCDEF fi</p>".to_owned();
        if scenario == "nested" {
            for _ in 0..3 {
                children = format!("<div style='display: flex;'>{children}{children}</div>");
            }
        }
        let children = children.repeat(count);
        let document = html::parse(&format!("<div id='root' style='{style}'>{children}</div>"))
            .map_err(|error| error.to_string())?;
        let root = document.find("root").unwrap();
        let text = document
            .walk()
            .into_iter()
            .filter(|node| {
                matches!(
                    document.entry(*node).unwrap().kind,
                    domengine::Kind::Text(_)
                )
            })
            .nth(count / 2)
            .unwrap();
        Ok(Self {
            document,
            scenario: scenario.into(),
            root,
            text,
            width: 128,
        })
    }

    pub fn update(&mut self, index: usize) -> Result<()> {
        match self.scenario.as_str() {
            "paint" => self.document.attribute(
                self.root,
                "style",
                if index.is_multiple_of(2) {
                    "color: #ff0000;"
                } else {
                    "color: #0000ff;"
                },
            ),
            "tiny" | "text" | "flex" | "nested" | "large" => {
                self.document.replace(self.text, &format!("Value {index}"))
            }
            "constraints" => {
                self.width = if index.is_multiple_of(2) { 64 } else { 128 };
                Ok(())
            }
            _ => unreachable!(),
        }
        .map_err(|error| error.to_string())
    }

    pub fn constraints(&self) -> Constraints {
        Constraints::viewport(Extent {
            width: self.width,
            height: 128,
        })
    }
}
