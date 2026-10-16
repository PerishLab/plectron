use clap::{Args, Parser, Subcommand};
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
    Ipc(Ipc),
    Frames(Frames),
    Passive(Passive),
    Activity(Activity),
    Feel(Feel),
    Correlate {
        source: PathBuf,
    },
    Record(Capture),
    Bench(Benchmark),
    Snap(Snap),
}

#[derive(Args)]
struct Snap {
    #[arg(long)]
    window: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Args)]
struct Ipc {
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    internal: PathBuf,
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 40)]
    runs: usize,
}

#[derive(Args)]
struct Frames {
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    apps: PathBuf,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 20)]
    runs: usize,
    #[arg(long, default_value_t = 0.0)]
    interval: f64,
}

#[derive(Args)]
struct Passive {
    #[arg(long)]
    binary: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    apps: PathBuf,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 20)]
    runs: usize,
}

#[derive(Args)]
struct Activity {
    #[arg(long)]
    binary: PathBuf,
    #[arg(long)]
    application: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, num_args = 1.., value_parser = ["cpu", "metal"], default_values_t = ["cpu".to_owned(), "metal".to_owned()])]
    backends: Vec<String>,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 2.0)]
    seconds: f64,
}

#[derive(Args)]
struct Feel {
    #[arg(long)]
    binary: PathBuf,
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "manual")]
    label: String,
    #[arg(long, value_parser = ["manual", "forward", "reversal"], default_value = "manual")]
    drive: String,
    #[arg(long, default_value_t = 20.0)]
    seconds: f64,
    #[arg(long, default_value_t = 2.0)]
    interval: f64,
}

#[derive(Args)]
struct Capture {
    plan: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "target/release/spec")]
    binary: PathBuf,
    #[arg(long, default_value_t = 30)]
    seconds: u64,
    #[arg(long)]
    limit: Option<u64>,
}

#[derive(Args)]
struct Benchmark {
    #[command(flatten)]
    capture: Capture,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long)]
    runs: Option<usize>,
    #[arg(long, num_args = 1.., value_parser = ["off", "wall", "cpu", "native"], default_values_t = ["off".to_owned(), "wall".to_owned(), "cpu".to_owned(), "native".to_owned()])]
    modes: Vec<String>,
}

#[locus::trace(with = observer.map(plectron_locus::Run::view))]
fn execute(
    command: Command,
    observer: Option<&plectron_locus::Run>,
) -> plectron_observe::Result<Value> {
    match command {
        Command::Report { source, output } => report::write(&source, &output),
        Command::Handoff { source, output } => handoff::write_summary(&source, &output),
        Command::Correlate { source } => trace::correlate(&source),
        Command::Ipc(value) => ipc::run(&ipc::Config {
            baseline: value.baseline,
            internal: value.internal,
            candidate: value.candidate,
            output: value.output,
            rounds: value.rounds,
            runs: value.runs,
        }),
        Command::Frames(value) => frames::run(&frames::Config {
            baseline: value.baseline,
            candidate: value.candidate,
            output: value.output,
            apps: value.apps,
            rounds: value.rounds,
            runs: value.runs,
            interval_ms: value.interval,
        }),
        Command::Passive(value) => passive::run(&passive::Config {
            binary: value.binary,
            output: value.output,
            apps: value.apps,
            rounds: value.rounds,
            runs: value.runs,
        }),
        Command::Activity(value) => activity::run(&activity::Config {
            binary: value.binary,
            application: value.application,
            output: value.output,
            backends: value.backends,
            rounds: value.rounds,
            seconds: value.seconds,
        }),
        Command::Feel(value) => feel::run(&feel::Config {
            binary: value.binary,
            project: value.project,
            output: value.output,
            label: value.label,
            drive: value.drive,
            seconds: value.seconds,
            interval_ms: value.interval,
        }),
        Command::Record(value) => native::record(&native::Config {
            plan: value.plan,
            output: value.output,
            binary: value.binary,
            seconds: value.seconds,
            limit: value.limit,
        }),
        Command::Snap(value) => snap::run(&snap::Config {
            window: value.window,
            output: value.output,
        }),
        Command::Bench(value) => bench::run(&bench::Config {
            plan: value.capture.plan,
            output: value.capture.output,
            binary: value.capture.binary,
            seconds: value.capture.seconds,
            limit: value.capture.limit,
            rounds: value.rounds,
            runs: value.runs,
            modes: value.modes,
        }),
    }
}

fn main() {
    if let Err(error) = plumb::identity!("PLECTRON_OBSERVE") {
        eprintln!("observe: {error}");
        std::process::exit(1);
    }
    let command = Cli::parse().command;
    let observer = plectron_locus::Run::start("observe");
    let code = match execute(command, observer.as_ref()) {
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
