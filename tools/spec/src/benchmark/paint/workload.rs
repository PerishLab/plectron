use crate::Result;
use ::fixtures::Text;
use domengine::{Constraints, Extent, Input, Layout, Sheet, html};

pub(super) fn build(scenario: &str) -> Result<Vec<Layout>> {
    let count = if scenario == "tiny" { 1 } else { 48 };
    let children = "<p style='height: 24px;'>ABCDEF fi</p>".repeat(count);
    let mut document = html::parse(&format!("<div id='root' style='width: 192px; height: 128px; overflow: hidden;'><p id='before' style='height: 16px;'></p>{children}</div>"))
        .map_err(|error| error.to_string())?;
    let root = document.find("root").unwrap();
    let before = document.find("before").unwrap();
    let child = document.entry(root).unwrap().children[1 + count / 2];
    let text = document.entry(child).unwrap().children[0];
    let mut layouts = Vec::new();
    for index in 0..68 {
        match scenario {
            "tiny" | "text" => document.replace(text, &format!("Value {index}")),
            "placement" => document.attribute(before, "style", &format!("height: {}px;", 16 + index)),
            "color" => document.attribute(child, "style", &format!("height: 24px; background-color: #{:06x};", 0x220000 + index * 0x101)),
            "transform" => document.attribute(root, "style", &format!("width: 192px; height: 128px; overflow: hidden; opacity: 0.5; translate: {}px -2px;", index)),
            _ => unreachable!(),
        }.map_err(|error| error.to_string())?;
        let input =
            Input::capture(&document, &Sheet::default()).map_err(|error| error.to_string())?;
        let layout = Layout::build(
            &input,
            Constraints::viewport(Extent {
                width: 256,
                height: 128,
            }),
            &Text,
        )
        .map_err(|error| error.to_string())?;
        layouts.push(layout);
    }
    Ok(layouts)
}
