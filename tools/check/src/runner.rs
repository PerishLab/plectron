use crate::{Plan, Result};
use std::path::Path;
use std::process::Command;

pub fn command(root: &Path, program: &str) -> Result<Command> {
    plumb::config::search(root)?.command(program)
}

pub fn execute(root: &Path, plan: &mut Plan) -> Result<()> {
    for gate in &mut plan.gates {
        let (program, arguments) = gate
            .command
            .split_first()
            .ok_or_else(|| format!("gate {} has no command", gate.id))?;
        eprintln!("check: {}", gate.id);
        let status = command(root, program)?
            .args(arguments)
            .status()
            .map_err(|error| error.to_string())?;
        gate.result = Some(status.success());
        if !status.success() {
            if let std::borrow::Cow::Borrowed(id) = gate.id {
                plumb::observation::fault(id);
            }
            return Err(format!("gate failed: {}", gate.id));
        }
    }
    Ok(())
}
