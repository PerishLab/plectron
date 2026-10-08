use crate::{Profile, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Component,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub entry: String,
    pub launcher: String,
    pub capabilities: Vec<String>,
    pub prefix: String,
    pub region: String,
    #[serde(default)]
    pub regions: Vec<String>,
    pub viewport: [usize; 2],
    pub sizes: BTreeMap<String, usize>,
    pub actions: Vec<String>,
}
pub fn load() -> Result<Vec<Case>> {
    let source = shared::Resource::new(include_bytes!("../../../../apps/catalog.json"));
    let cases: Vec<Case> = serde_json::from_str(source.text().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    validate(&cases)?;
    Ok(cases)
}
pub fn validate(cases: &[Case]) -> Result<()> {
    let profile = Profile::load()?;
    let registry = fixtures::registry::Registry::open(&crate::root())?;
    let mut ids = BTreeSet::new();
    for case in cases {
        if !ids.insert(&case.id) || case.prefix.is_empty() || case.actions.is_empty() {
            return Err("invalid scenario identity or actions".into());
        }
        let baseline = ["append", "edit", "reorder", "remove"];
        if !case.actions.iter().map(String::as_str).take(4).eq(baseline) {
            return Err("scenario baseline requires append, edit, reorder, remove in order".into());
        }
        if case.actions.iter().collect::<BTreeSet<_>>().len() != case.actions.len() {
            return Err("duplicate scenario action".into());
        }
        if !case.regions.is_empty()
            && (!case.regions.contains(&case.region)
                || case.regions.iter().collect::<BTreeSet<_>>().len() != case.regions.len())
        {
            return Err("scenario regions must be unique and include the primary region".into());
        }
        asset(&case.entry)?;
        asset(&case.launcher)?;
        registry.workload(&case.id, &case.entry, &case.launcher)?;
        for capability in &case.capabilities {
            if !profile
                .capabilities
                .iter()
                .any(|c| c.id == *capability && c.state == crate::profile::State::Subset)
            {
                return Err(format!(
                    "scenario {} requires unavailable capability {capability}",
                    case.id
                ));
            }
        }
        graphics::shared::Extent {
            width: case.viewport[0],
            height: case.viewport[1],
        }
        .area()
        .map_err(|e| e.to_string())?;
        if case.sizes.is_empty() || case.sizes.values().any(|n| !(2..=4096).contains(n)) {
            return Err("scenario size requires 2 through 4096 records".into());
        }
        if case.actions.iter().any(|a| {
            ![
                "append", "edit", "reorder", "remove", "width", "font", "focus", "scroll", "swap",
                "restore", "fade", "shift", "hide", "show", "receive", "resume", "switch",
            ]
            .contains(&a.as_str())
        }) {
            return Err("unknown scenario action".into());
        }
    }
    Ok(())
}
fn asset(name: &str) -> Result<()> {
    if name.is_empty()
        || std::path::Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("scenario asset must be relative to apps".into());
    }
    if !crate::root().join("apps").join(name).is_file() {
        return Err(format!("missing scenario asset: {name}"));
    }
    Ok(())
}
