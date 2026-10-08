use clap::{Parser, Subcommand, ValueEnum};
use plectron_chromium::{
    Result, artifact, compare, digest, paired, prerequisite, read, reference, summary, write,
    write_new,
};
use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Parser)]
#[command(
    name = "plectron-chromium",
    about = "Capture and analyze Plectron workloads in local Chromium.",
    version = plumb::version!("PLECTRON_CHROMIUM")
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Measure a scenario through playwright-cli.
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
    /// Summarize an existing Chromium capture and trace.
    Summary { directory: PathBuf },
    /// Freeze compatible observation batches as a provisional reference.
    Reference {
        #[arg(required = true)]
        directories: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Compare a native report with a paired Chromium capture.
    Compare {
        native: PathBuf,
        chromium: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Scenario {
    Article,
    Cards,
    Desk,
    Gallery,
    Isolation,
    Layers,
    List,
}

impl Scenario {
    fn name(self) -> &'static str {
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
    match run(Cli::parse()) {
        Ok(success) if success => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("chromium: {error}");
            ExitCode::from(2)
        }
    }
}

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
            measure(&output, count, runs, warmups, paired, scenario)?;
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
            write_new(&output, &reference::freeze(&directories)?)?;
            Ok(true)
        }
        Action::Compare {
            native,
            chromium,
            output,
        } => {
            let result = compare::compare(&native, &chromium)?;
            write_new(&output, &result)?;
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

fn measure(
    requested: &Path,
    count: u64,
    runs: u64,
    warmups: u64,
    paired_capture: bool,
    scenario: Scenario,
) -> Result<()> {
    if count < 2 || runs < 1 {
        return Err("count >= 2 and runs >= 1 required".into());
    }
    if !matches!(scenario, Scenario::Cards) && !paired_capture {
        return Err("additional scenarios require --paired".into());
    }
    let output = absolute(requested)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::create_dir(&output).map_err(|error| format!("{}: {error}", output.display()))?;
    prerequisite::record(
        &output,
        command_output(
            Command::new("playwright-cli")
                .arg("--version")
                .current_dir(&output),
            TIMEOUT,
        ),
    )?;
    let session = format!("plectron-{}-{}", scenario.name(), std::process::id());
    let log = File::create(output.join("serve.log")).map_err(|error| error.to_string())?;
    let mut server = Command::new(server_binary()?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| format!("could not start plectron-serve: {error}"))?;
    let result = measure_with_server(
        &mut server,
        &output,
        &session,
        count,
        runs,
        warmups,
        paired_capture,
        scenario,
    );
    terminate(&mut server);
    result
}

#[allow(clippy::too_many_arguments)]
fn measure_with_server(
    server: &mut Child,
    output: &Path,
    session: &str,
    count: u64,
    runs: u64,
    warmups: u64,
    paired_capture: bool,
    scenario: Scenario,
) -> Result<()> {
    let stdout = server
        .stdout
        .take()
        .ok_or("static server has no readiness channel")?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let line = receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "static server did not announce readiness".to_owned())?
        .map_err(|error| error.to_string())?;
    let ready: Value =
        serde_json::from_str(&line).map_err(|error| format!("server readiness: {error}"))?;
    let mut config = json!({
        "url": ready["url"],
        "output": output,
        "count": count,
        "runs": runs,
        "warmups": warmups,
        "scenario": scenario.name(),
    });
    let root = repository_root();
    let scripts = artifact::scripts(&root.join("apps"), scenario.name())?;
    config["order"] = json!(artifact::entries(&root.join("apps"), scenario.name())?);
    config["scripts"] = json!(
        scripts
            .iter()
            .map(|(name, artifact)| (name.clone(), artifact["body"].clone()))
            .collect::<Map<_, _>>()
    );
    if paired_capture {
        config["stylesheet"] =
            Value::String(read_text(&root.join("apps/benchmarks/normalize.css"))?);
        let catalog = read(&root.join("apps/catalog.json"))?;
        let case = catalog
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["id"] == scenario.name()))
            .ok_or_else(|| format!("scenario is absent from catalog: {}", scenario.name()))?;
        for name in ["actions", "prefix", "region", "regions"] {
            if let Some(value) = case.get(name) {
                config[name] = value.clone();
            }
        }
        if config.get("regions").is_none() {
            config["regions"] = json!([case["region"]]);
        }
    }
    write(&output.join("config.json"), &config)?;
    let harness = root.join("tools/chromium").join(if paired_capture {
        "paired.js"
    } else {
        "cards.js"
    });
    let source = read_text(&harness)?;
    let encoded = serde_json::to_string(&config).map_err(|error| error.to_string())?;
    let script = output.join("run.js");
    fs::write(&script, source.replace("__CONFIG__", &encoded))
        .map_err(|error| error.to_string())?;
    eprintln!("Starting Chromium {} measurement", scenario.name());
    playwright(session, &["open".into(), "--browser=chrome".into()], output)?;
    let capture = (|| {
        let result = playwright(
            session,
            &[
                "run-code".into(),
                format!("--filename={}", script.display()),
            ],
            output,
        )?;
        let mut report: Value =
            serde_json::from_str(&result).map_err(|error| format!("browser report: {error}"))?;
        report["prerequisites"] = read(&output.join("prerequisites.json"))?;
        report["artifacts"] = artifact::verify(&scripts, &report["scripts"])?;
        report["translator"] = json!({
            "sha256": digest(&fs::read(std::env::current_exe().map_err(|error| error.to_string())?).map_err(|error| error.to_string())?),
            "debug": cfg!(debug_assertions),
            "boundary": "Rust project translator; exact emitted Script bytes consumed in ordered browser workload scope",
        });
        report["worktree"] = Value::String(command_text("git", &["status", "--porcelain"], &root)?);
        report
            .as_object_mut()
            .ok_or("browser report must be an object")?
            .remove("scripts");
        report["host"] = json!({
            "system": command_text("uname", &["-a"], &root)?.trim(),
            "architecture": std::env::consts::ARCH,
        });
        report["revision"] = Value::String(
            command_text("git", &["rev-parse", "HEAD"], &root)?
                .trim()
                .into(),
        );
        let mut paths = vec![
            harness.clone(),
            root.join("apps").join(scenario.name()).join("index.html"),
        ];
        if paired_capture {
            let requests: Vec<_> = report["requests"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            report["resources"] = paired::resources(
                &root.join("apps"),
                scenario.name(),
                config["stylesheet"].as_str().unwrap_or_default(),
                &requests,
            )?;
            for (name, artifact) in report["artifacts"]
                .as_object()
                .ok_or("missing script artifacts")?
            {
                if ["bytes", "fingerprint"]
                    .into_iter()
                    .any(|key| report["resources"][name][key] != artifact[key])
                {
                    return Err(format!("browser script changed during run: {name}"));
                }
            }
            paths.extend(
                report["resources"]
                    .as_object()
                    .into_iter()
                    .flat_map(Map::keys)
                    .map(|name| root.join("apps").join(name)),
            );
            paths.push(root.join("apps/catalog.json"));
        } else {
            paths.extend(
                ["index.html", "style.css"]
                    .map(|name| root.join("apps").join(scenario.name()).join(name)),
            );
            paths.push(root.join("apps/fonts/Lato-Regular.ttf"));
        }
        for script in scripts.values() {
            paths.extend(
                script["sources"]
                    .as_object()
                    .ok_or("missing script source identities")?
                    .keys()
                    .map(|name| root.join(name)),
            );
        }
        report["sources"] = Value::Object(
            paths
                .into_iter()
                .map(|path| {
                    let data =
                        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                    let name = path
                        .strip_prefix(&root)
                        .map_err(|error| error.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/");
                    Ok((name, json!(digest(&data))))
                })
                .collect::<Result<_>>()?,
        );
        if paired_capture {
            paired::save(output, &mut report)?;
        }
        write(&output.join("report.json"), &report)?;
        if !paired_capture {
            summary::summarize(output)?;
        }
        println!("{}", output.join("report.json").display());
        Ok(())
    })();
    let close = playwright(session, &["close".into()], output).map(|_| ());
    capture.and(close)
}

fn server_binary() -> Result<PathBuf> {
    let root = repository_root();
    let output = command_output(
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "-p",
                "plectron-serve",
                "--message-format=json",
            ])
            .current_dir(&root),
        TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&[output.stdout, output.stderr].concat()).into_owned());
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(artifact) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if artifact["reason"] == "compiler-artifact"
            && artifact["target"]["name"] == "plectron-serve"
            && let Some(executable) = artifact["executable"].as_str()
        {
            return Ok(executable.into());
        }
    }
    Err("cargo did not produce the plectron-serve executable".into())
}

fn playwright(session: &str, args: &[String], output: &Path) -> Result<String> {
    let result = command_output(
        Command::new("playwright-cli")
            .arg(format!("-s={session}"))
            .arg("--raw")
            .args(args)
            .current_dir(output),
        TIMEOUT,
    )?;
    let combined = [result.stdout.as_slice(), result.stderr.as_slice()].concat();
    fs::write(output.join(format!("{}.log", args[0])), &combined)
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8(result.stdout).map_err(|error| error.to_string())?;
    if !result.status.success() || stdout.starts_with("### Error") {
        return Err(String::from_utf8_lossy(&combined).into_owned());
    }
    Ok(stdout)
}

fn command_text(program: &str, args: &[&str], directory: &Path) -> Result<String> {
    let output = command_output(
        Command::new(program).args(args).current_dir(directory),
        TIMEOUT,
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn command_output(command: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("command has no stdout")?;
    let stderr = child.stderr.take().ok_or("command has no stderr")?;
    let out = thread::spawn(move || read_all(stdout));
    let err = thread::spawn(move || read_all(stderr));
    let status = match child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("command exceeded {} seconds", timeout.as_secs()));
        }
    };
    Ok(std::process::Output {
        status,
        stdout: out.join().map_err(|_| "stdout reader panicked")??,
        stderr: err.join().map_err(|_| "stderr reader panicked")??,
    })
}

fn read_all(mut input: impl Read) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    input
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    Ok(data)
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGTERM,
    );
    #[cfg(not(unix))]
    let _ = child.kill();
    if child
        .wait_timeout(Duration::from_secs(5))
        .ok()
        .flatten()
        .is_none()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.into())
    } else {
        std::env::current_dir()
            .map(|root| root.join(path))
            .map_err(|error| error.to_string())
    }
}

fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .components()
        .collect()
}
