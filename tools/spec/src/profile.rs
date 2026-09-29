use crate::Result;
use serde::{Deserialize, Serialize};
use shared::Resource;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const PROFILE: Resource = Resource::new(include_bytes!("../resources/profile.json"));

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub reference: usize,
    pub specifications: Vec<Specification>,
    pub capabilities: Vec<Capability>,
    pub suite: Suite,
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    pub id: String,
    pub url: String,
    pub revision: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub area: Area,
    pub state: State,
    pub specification: String,
    pub section: String,
    pub scope: String,
    pub gaps: Vec<String>,
    pub implementation: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Area {
    Html,
    Css,
    Js,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Subset,
    Pending,
    Excluded,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub url: String,
    pub revision: String,
    pub license: String,
    pub blob: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub path: String,
    pub file: String,
    pub blob: String,
    pub capability: String,
    pub phase: Phase,
    pub mode: Mode,
    pub selection: Selection,
    pub reason: Option<Reason>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Parse,
    Runtime,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Strict,
    Sloppy,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Selection {
    Run,
    Excluded,
    Unsupported,
    Blocked,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Legacy,
    Semantics,
    Syntax,
    Harness,
}

impl Profile {
    pub fn matrix(&self) -> BTreeMap<Area, Vec<&Capability>> {
        let mut areas: BTreeMap<Area, Vec<&Capability>> = BTreeMap::new();
        for capability in &self.capabilities {
            areas.entry(capability.area).or_default().push(capability);
        }
        areas
    }
    pub fn load() -> Result<&'static Self> {
        static CACHE: OnceLock<Result<Profile>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                let source = PROFILE.text().map_err(|error| error.to_string())?;
                let profile: Self =
                    serde_json::from_str(source).map_err(|error| error.to_string())?;
                profile.validate()?;
                Ok(profile)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for capability in &self.capabilities {
            capability.validate()?;
            if !ids.insert(&capability.id) {
                return Err("duplicate capability".into());
            }
            if !self
                .specifications
                .iter()
                .any(|spec| spec.id == capability.specification)
            {
                return Err("unknown specification".into());
            }
        }
        let mut files = BTreeSet::new();
        for case in &self.cases {
            if !files.insert(&case.file) || !ids.contains(&case.capability) {
                return Err("invalid case ownership".into());
            }
            if (case.selection == Selection::Run) != case.reason.is_none() {
                return Err("case reason disagrees with selection".into());
            }
        }
        Ok(())
    }
}

impl Capability {
    fn validate(&self) -> Result<()> {
        if (self.state == State::Pending) != self.scope.is_empty() {
            return Err(format!(
                "capability scope disagrees with state: {}",
                self.id
            ));
        }
        if self.state == State::Subset
            && (self.implementation.is_empty() || self.evidence.is_empty())
        {
            return Err(format!(
                "implemented subset needs source and evidence: {}",
                self.id
            ));
        }
        for path in self.implementation.iter().chain(&self.evidence) {
            if !crate::root().join(path).is_file() {
                return Err(format!("missing capability evidence: {path}"));
            }
        }
        Ok(())
    }
}
