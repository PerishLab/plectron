use clap::{Parser, Subcommand, ValueEnum};
mod measure;
mod process;

use measure::{Host, Measurement};
use plectron_chromium::{Result, compare, create, reference, summary};
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "plectron-chromium",
    about = "Capture and analyze Plectron workloads in local Chromium.",
    version = plumb::version!("PLECTRON_CHROMIUM"), after_help = plectron_locus::HELP
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    #[command(about = "Measure a scenario through playwright-cli.")]
    Measure {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 384)]
        count: u64,
        #[arg(long, default_value_t = 5)]
        runs: u64,
        #[arg(long, default_value_t = 2)]
        warmups: u64,
        #[arg(long)]
        paired: bool,
        #[arg(long, value_enum, default_value_t = Scenario::Cards)]
        scenario: Scenario,
    },
    #[command(about = "Summarize an existing Chromium capture and trace.")]
    Summary { directory: PathBuf },
    #[command(about = "Freeze compatible observation batches as a provisional reference.")]
    Reference {
        #[arg(required = true)]
        directories: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    #[command(about = "Compare a native report with a paired Chromium capture.")]
    Compare {
        native: PathBuf,
        chromium: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Scenario {
    Article,
    Cards,
    Desk,
    Gallery,
    Isolation,
    Layers,
    List,
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Cards => "cards",
            Self::Desk => "desk",
            Self::Gallery => "gallery",
            Self::Isolation => "isolation",
            Self::Layers => "layers",
            Self::List => "list",
        }
    }
}

fn main() -> ExitCode {
    if let Err(error) = plumb::identity!("PLECTRON_CHROMIUM") {
        eprintln!("chromium: {error}");
        return ExitCode::from(1);
    }
    let cli = Cli::parse();
    let observer = plectron_locus::start("chromium");
    let code = match run(cli) {
        Ok(success) if success => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("chromium: {error}");
            ExitCode::from(2)
        }
    };
    if let Some(observer) = observer {
        observer.finish(if code == ExitCode::SUCCESS {
            0
        } else if code == ExitCode::from(1) {
            1
        } else {
            2
        });
    }
    code
}

#[locus::trace(with = plumb::observation::view())]
fn run(cli: Cli) -> Result<bool> {
    match cli.command {
        Action::Measure {
            output,
            count,
            runs,
            warmups,
            paired,
            scenario,
        } => {
            Measurement {
                output: absolute(&output)?,
                count,
                runs,
                warmups,
                paired,
                scenario,
                host: Host {
                    executable: std::env::current_exe().map_err(|error| error.to_string())?,
                    architecture: std::env::consts::ARCH,
                },
            }
            .measure()?;
            Ok(true)
        }
        Action::Summary { directory } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summary::summarize(&directory)?)
                    .map_err(|error| error.to_string())?
            );
            Ok(true)
        }
        Action::Reference {
            directories,
            output,
        } => {
            create(&output, &reference::freeze(&directories)?)?;
            Ok(true)
        }
        Action::Compare {
            native,
            chromium,
            output,
        } => {
            let result = compare::compare(&native, &chromium)?;
            create(&output, &result)?;
            println!(
                "{}",
                json!({
                    "passed": result["passed"],
                    "issues": result["issues"],
                    "classification": result["classification"],
                })
            );
            Ok(result["passed"].as_bool() == Some(true))
        }
    }
}

fn absolute(path: &std::path::Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.into())
    } else {
        std::env::current_dir()
            .map(|root| root.join(path))
            .map_err(|error| error.to_string())
    }
}
