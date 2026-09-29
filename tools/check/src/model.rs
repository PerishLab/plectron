use crate::Change;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Domain {
    Crate(String),
    Consumer(String),
    Input,
    Graphics,
    Typography,
    Script,
    Http,
    Profile,
    Tooling,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Cost {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gate {
    pub id: String,
    pub command: Vec<String>,
    pub cost: Cost,
    pub reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub mode: String,
    pub changes: Vec<Change>,
    pub domains: Vec<Domain>,
    pub gates: Vec<Gate>,
    pub skipped: Vec<String>,
    pub escalation: Option<String>,
}

impl Plan {
    pub fn text(&self) -> String {
        let mut lines = vec![format!(
            "mode={} changes={} gates={}",
            self.mode,
            self.changes.len(),
            self.gates.len()
        )];
        if let Some(reason) = &self.escalation {
            lines.push(format!("escalation: {reason}"));
        }
        lines.push(format!(
            "domains: {}",
            self.domains
                .iter()
                .map(|domain| format!("{domain:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for gate in &self.gates {
            lines.push(format!(
                "select {} [{:?}]: {}",
                gate.id,
                gate.cost,
                gate.reasons.join("; ")
            ));
            lines.push(format!("  {}", gate.command.join(" ")));
        }
        for skipped in &self.skipped {
            lines.push(format!("skip {skipped}"));
        }
        lines.join("\n")
    }
}
