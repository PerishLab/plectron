use clap::{Parser, Subcommand};
use plectron_observe::{
    activity, bench, feel, frames, handoff, ipc, native, passive, report, snap, trace,
};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "observe", version = plumb::version!("PLECTRON_OBSERVE"), after_help = plectron_locus::HELP)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Report {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Handoff {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Ipc(ipc::Config),
    Frames(frames::Config),
    Passive(passive::Config),
    Activity(activity::Config),
    Feel(feel::Config),
    Correlate {
        source: PathBuf,
    },
    Record(native::Config),
    Bench(bench::Config),
    Snap(snap::Config),
}

#[locus::trace(with = plumb::observation::view())]
fn execute(command: Command) -> plectron_observe::Result<Value> {
    match command {
        Command::Report { source, output } => report::write(&source, &output),
        Command::Handoff { source, output } => handoff::summarize(&source, &output),
        Command::Correlate { source } => trace::correlate(&source),
        Command::Ipc(config) => ipc::run(&config),
        Command::Frames(config) => frames::run(&config),
        Command::Passive(config) => passive::run(&config),
        Command::Activity(config) => activity::run(&config),
        Command::Feel(config) => feel::run(&config),
        Command::Record(config) => native::record(&config),
        Command::Snap(mut config) => {
            config.display = std::env::var("DISPLAY").ok();
            snap::run(&config)
        }
        Command::Bench(config) => bench::run(&config),
    }
}

fn main() {
    if let Err(error) = plumb::identity!("PLECTRON_OBSERVE") {
        eprintln!("observe: {error}");
        std::process::exit(1);
    }
    let command = Cli::parse().command;
    let observer = plectron_locus::start("observe");
    let code = match execute(command) {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            0
        }
        Err(error) => {
            eprintln!("observe: {error}");
            1
        }
    };
    if let Some(observer) = observer {
        observer.finish(code);
    }
    if code != 0 {
        std::process::exit(code);
    }
}
