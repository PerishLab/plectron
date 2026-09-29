use crate::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use webengine::Snapshot;

#[derive(Default, Serialize)]
pub struct Geometry {
    pub boxes: BTreeMap<String, Bounds>,
    pub scroll: usize,
    pub extent: usize,
}

pub type Bounds = [i64; 4];

pub(in crate::scenario) fn capture(
    snapshot: &Snapshot,
    case: &super::super::Case,
) -> Result<Geometry> {
    let view = super::View(snapshot);
    let root = view.node(&case.region)?;
    let mut nodes = vec![("region".to_string(), root.id)];
    nodes.extend(
        snapshot
            .nodes
            .iter()
            .filter(|node| node.parent == Some(root.id))
            .take(3)
            .enumerate()
            .map(|(index, node)| (format!("record-{index}"), node.id)),
    );
    let mut boxes = BTreeMap::new();
    for (name, id) in nodes {
        if let Some(fragment) = snapshot
            .fragments
            .iter()
            .find(|f| f.node == Some(id) && matches!(f.content, domengine::Content::Box { .. }))
        {
            let mapping = snapshot
                .spaces
                .iter()
                .find(|space| space.id == fragment.space)
                .ok_or("geometry space missing")?;
            let rect = fragment.rect;
            boxes.insert(
                name,
                [
                    mapping.translation[0],
                    mapping.translation[1],
                    rect.width as i64,
                    rect.height as i64,
                ],
            );
        }
    }
    let space = view.scroller(&case.region)?;
    let mapping = snapshot
        .spaces
        .iter()
        .find(|entry| entry.id == space)
        .ok_or("geometry scroller missing")?;
    Ok(Geometry {
        boxes,
        scroll: snapshot.visual.offset(space),
        extent: mapping.range.unwrap_or(0) + mapping.clip.height,
    })
}
