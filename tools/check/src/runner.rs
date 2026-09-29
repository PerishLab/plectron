use crate::{Plan, Result};
use std::path::Path;
use std::process::Command;

pub fn execute(root: &Path, plan: &mut Plan) -> Result<()> {
    for gate in &mut plan.gates {
        let (program, arguments) = gate
            .command
            .split_first()
            .ok_or_else(|| format!("gate {} has no command", gate.id))?;
        eprintln!("check: {}", gate.id);
        let status = Command::new(program)
            .args(arguments)
            .current_dir(root)
            .status()
            .map_err(|error| error.to_string())?;
        gate.result = Some(status.success());
        if !status.success() {
            return Err(format!("gate failed: {}", gate.id));
        }
    }
    Ok(())
}
