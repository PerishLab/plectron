use clap::Parser;
use plectron_check::{Source, execute, plan};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "plectron-check", version = plumb::version!("PLECTRON_CHECK"))]
struct Cli {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    arguments: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("check: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = root()?;
    let mut args = Cli::parse().arguments.into_iter();
    let action = args.next().unwrap_or_else(|| "plan".into());
    let mut source = Source::Staged;
    let mut full = false;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--staged" => source = Source::Staged,
            "--worktree" => source = Source::Worktree,
            "--range" => source = Source::Range(args.next().ok_or("--range requires a value")?),
            "--input" => {
                source = Source::File(PathBuf::from(args.next().ok_or("--input requires a path")?))
            }
            "--full" => full = true,
            "--json" => json = true,
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if action != "plan" && action != "run" {
        return Err("usage: cargo verify [plan|run] [--staged|--worktree|--range RANGE|--input FILE] [--full] [--json]".into());
    }
    let mut plan = plan(&root, &source, full)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
        );
    } else {
        println!("{}", plan.text());
    }
    if action == "run" {
        execute(&root, &mut plan)?;
    }
    Ok(())
}

fn root() -> Result<PathBuf, String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let current = std::env::current_dir().map_err(|error| error.to_string())?;
        if current.join("Cargo.toml").is_file() {
            return Ok(current);
        }
        return Err("current directory is not a Cargo or Git workspace".into());
    }
    let path = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    Ok(Path::new(path.trim()).to_path_buf())
}
