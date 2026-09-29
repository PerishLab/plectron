use crate::Result;
use std::collections::BTreeMap;
use webengine::Snapshot;

mod geometry;
pub use geometry::Geometry;
pub(super) use geometry::capture;

#[derive(Debug, PartialEq)]
struct Record {
    id: usize,
    parent: Option<usize>,
    text: String,
}

pub(super) fn step(
    data: (&Snapshot, &Snapshot),
    case: &super::Case,
    action: &str,
    batch: usize,
) -> Result<()> {
    let prefix = &case.prefix;
    let before = View(data.0).records(prefix);
    let after = View(data.1).records(prefix);
    let expected = match action {
        "append" | "receive" => before.len() + batch,
        "remove" => before.len() - 1,
        _ => before.len(),
    };
    let valid = if action == "switch" {
        !after.is_empty() && View(data.1).count(prefix) == after.len()
    } else {
        after.len() == expected && View(data.1).count(prefix) == expected
    };
    if !valid {
        return Err(format!("{action}: wrong record count"));
    }
    if action == "append" && (before.len()..expected).any(|id| !after.contains_key(&id.to_string()))
    {
        return Err("append did not create the requested identities".into());
    }
    for (key, record) in &before {
        if action == "remove" && key == "1" {
            if after.contains_key(key) {
                return Err("removed record remains connected".into());
            }
            continue;
        }
        if action == "switch" {
            let Some(next) = after.get(key) else { continue };
            if record.id == next.id || record.parent == next.parent || record.text == next.text {
                return Err("switch did not replace the channel records in place".into());
            }
            continue;
        }
        let next = after.get(key).ok_or("unrelated record disappeared")?;
        if record.id != next.id || record.parent != next.parent {
            return Err("operation changed retained node identity or parent".into());
        }
        if action == "edit" && key.parse::<usize>().is_ok_and(|id| id < batch) {
            if record.text == next.text || !next.text.contains("Changed") {
                return Err("edit did not change target text".into());
            }
        } else if record.text != next.text {
            return Err("operation changed unrelated text".into());
        }
    }
    special(data, case, action, batch)?;
    if action == "width" && View(data.0).width(&case.region)? == View(data.1).width(&case.region)? {
        return Err("width action did not change the region layout constraint".into());
    }
    Ok(())
}
fn special(
    data: (&Snapshot, &Snapshot),
    case: &super::Case,
    action: &str,
    batch: usize,
) -> Result<()> {
    let prefix = &case.prefix;
    match action {
        "reorder" => {
            let target = View(data.1).node(&format!("{prefix}0"))?;
            let last = data.1.nodes.iter().rfind(|n| {
                n.parent == target.parent
                    && n.attributes
                        .get("id")
                        .is_some_and(|id| id.starts_with(prefix))
            });
            if last.map(|n| n.id) != Some(target.id) {
                return Err("reorder did not move target to sibling end".into());
            }
        }
        "swap" | "restore" => pictures(data)?,
        "focus" => {
            if data.1.focused != Some(View(data.1).node("right-input")?.id) {
                return Err("focus did not move to right panel".into());
            }
        }
        "scroll" => {
            let changed = data
                .1
                .visual
                .offsets
                .iter()
                .filter(|(id, value)| {
                    data.0.visual.offsets.get(id).copied().unwrap_or(0) != **value
                })
                .count();
            let target = View(data.1).scroller(&case.region)?;
            let before = data.0.visual.offset(target);
            let after = data.1.visual.offset(target);
            if changed != 1 || data.0.revision != data.1.revision || before.abs_diff(after) != 48 {
                return Err(format!(
                    "wheel must move one visual space by 48 without document mutation: {changed} spaces, revision {:?} -> {:?}, offsets {:?} -> {:?}",
                    data.0.revision, data.1.revision, data.0.visual.offsets, data.1.visual.offsets
                ));
            }
        }
        "receive" => {
            let before = View(data.0).scroller(&case.region)?;
            let after = View(data.1).scroller(&case.region)?;
            if data.0.visual.offset(before) != data.1.visual.offset(after) {
                return Err("incoming messages moved the detached timeline".into());
            }
            let latest = View(data.1).node("latest")?;
            if latest.attributes["class"].contains("latesthidden") {
                return Err("incoming messages did not publish unread feedback".into());
            }
            let feedback = data
                .1
                .nodes
                .iter()
                .filter(|node| node.parent == Some(latest.id) && node.kind == "text")
                .map(|node| node.content.as_str())
                .collect::<String>();
            if feedback != format!("{batch} new messages ↓") {
                return Err("incoming message count did not reach unread feedback".into());
            }
        }
        "resume" => {
            let before = View(data.0).scroller(&case.region)?;
            let after = View(data.1).scroller(&case.region)?;
            if data.0.visual.offset(before) >= data.1.visual.offset(after) {
                return Err("unread feedback did not resume the timeline".into());
            }
            let latest = View(data.1).node("latest")?;
            if !latest.attributes["class"].contains("latesthidden") {
                return Err("resumed timeline retained unread feedback".into());
            }
        }
        "switch" => {
            let active = |id| {
                View(data.1)
                    .node(id)
                    .map(|node| node.attributes["class"].contains("channelactive"))
            };
            if active("general-channel")? || !active("engine-channel")? {
                return Err("switch did not publish the active channel state".into());
            }
        }
        "width" | "font" | "fade" | "shift" | "hide" | "show"
            if View(data.0).node(&case.region)?.attributes
                == View(data.1).node(&case.region)?.attributes =>
        {
            return Err("scenario region style did not change".into());
        }
        _ => (),
    }
    Ok(())
}

fn pictures(data: (&Snapshot, &Snapshot)) -> Result<()> {
    let sources = |snapshot: &Snapshot| {
        snapshot
            .nodes
            .iter()
            .filter(|node| node.kind == "element" && node.content == "img")
            .map(|node| (node.id, node.attributes.get("src").cloned()))
            .collect::<BTreeMap<_, _>>()
    };
    let before = sources(data.0);
    let after = sources(data.1);
    if before.is_empty()
        || !before.keys().eq(after.keys())
        || before
            .iter()
            .any(|(id, source)| after.get(id) == Some(source))
    {
        return Err("source action must change every retained picture source".into());
    }
    if data.1.images.bindings.checked_sub(data.0.images.bindings) != Some(before.len() as u64) {
        return Err("source action must rebind every retained picture".into());
    }
    Ok(())
}

pub(super) struct View<'a>(pub &'a Snapshot);
impl<'a> View<'a> {
    fn records(&self, prefix: &str) -> BTreeMap<String, Record> {
        self.0
            .nodes
            .iter()
            .filter_map(|node| {
                let name = node.attributes.get("id")?.strip_prefix(prefix)?;
                let text = self
                    .0
                    .nodes
                    .iter()
                    .filter(|child| child.parent == Some(node.id) && child.kind == "text")
                    .map(|child| child.content.as_str())
                    .collect();
                Some((
                    name.into(),
                    Record {
                        id: node.id,
                        parent: node.parent,
                        text,
                    },
                ))
            })
            .collect()
    }
    fn width(&self, name: &str) -> Result<usize> {
        let node = self.node(name)?;
        self.0
            .fragments
            .iter()
            .find(|fragment| {
                fragment.node == Some(node.id)
                    && matches!(fragment.content, domengine::Content::Box { .. })
            })
            .map(|fragment| fragment.rect.width)
            .ok_or_else(|| "region box missing".into())
    }
    pub(super) fn count(&self, prefix: &str) -> usize {
        self.0
            .nodes
            .iter()
            .filter(|node| {
                node.attributes
                    .get("id")
                    .is_some_and(|id| id.starts_with(prefix))
            })
            .count()
    }
    pub(super) fn seed(&self, prefix: &str, count: usize) -> Result<()> {
        let actual = self.records(prefix);
        if actual.len() != count
            || self.count(prefix) != count
            || (0..count).any(|id| !actual.contains_key(&id.to_string()))
        {
            return Err("seed must create exactly the requested records".into());
        }
        Ok(())
    }
    pub(super) fn node(&self, id: &str) -> Result<&'a webengine::Node> {
        self.0
            .nodes
            .iter()
            .find(|node| node.attributes.get("id").is_some_and(|name| name == id))
            .map(AsRef::as_ref)
            .ok_or_else(|| format!("missing node {id}"))
    }
    pub(super) fn scroller(&self, id: &str) -> Result<u64> {
        let node = self.node(id)?;
        let fragment = self
            .0
            .fragments
            .iter()
            .find(|f| f.node == Some(node.id))
            .ok_or("scroll fragment missing")?;
        self.0
            .spaces
            .iter()
            .find(|m| m.parent == Some(fragment.space) && m.range.is_some())
            .map(|m| m.id)
            .ok_or_else(|| "scroll content space missing".into())
    }
    pub(super) fn depth(&self) -> usize {
        let mut depths = BTreeMap::new();
        for node in self.0.nodes.iter() {
            let depth = node
                .parent
                .and_then(|p| depths.get(&p))
                .copied()
                .unwrap_or(0)
                + 1;
            depths.insert(node.id, depth);
        }
        depths.values().copied().max().unwrap_or(0)
    }
}
