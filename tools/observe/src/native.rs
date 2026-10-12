use crate::{Result, absolute, create_output, report, trace, write_new};
#[cfg(unix)]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::{Value, json};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub struct Config {
    pub plan: PathBuf,
    pub output: PathBuf,
    pub binary: PathBuf,
    pub seconds: u64,
    pub limit: Option<u64>,
}

pub fn invoke(command: &mut Command, log: &Path, timeout: Duration) -> Result<()> {
    let stream = File::create(log).map_err(|error| format!("{}: {error}", log.display()))?;
    let error = stream.try_clone().map_err(|error| error.to_string())?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stream))
        .stderr(Stdio::from(error))
        .spawn()
        .map_err(|error| error.to_string())?;
    let status = match child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "command exceeded {} seconds; see {}",
                timeout.as_secs_f64(),
                log.display()
            ));
        }
    };
    if !status.success() {
        return Err(format!("command failed ({status}); see {}", log.display()));
    }
    Ok(())
}

pub fn export(source: &Path, schema: &str, target: &Path) -> Result<()> {
    let mut command = Command::new("xcrun");
    command
        .args(["xctrace", "export", "--input"])
        .arg(source)
        .args([
            "--xpath",
            &format!("/trace-toc/run[@number=\"1\"]/data/table[@schema=\"{schema}\"]"),
            "--output",
        ])
        .arg(target);
    invoke(
        &mut command,
        &target.with_extension("log"),
        Duration::from_secs(60),
    )
}

pub fn record(config: &Config) -> Result<Value> {
    prerequisite()?;
    if config.seconds == 0 {
        return Err("seconds must be positive".into());
    }
    let root = create_output(&config.output)?;
    let binary = absolute(&config.binary)?;
    let plan = absolute(&config.plan)?;
    let source = root.join("cpu.trace");
    let target = root.join("scenario");
    let mut options = json!({"native": true, "cpu": true});
    if let Some(limit) = config.limit {
        options["limit"] = json!(limit);
    }
    let arguments = vec![
        "xctrace".into(),
        "record".into(),
        "--template".into(),
        "CPU Counters".into(),
        "--output".into(),
        source.to_string_lossy().into_owned(),
        "--time-limit".into(),
        format!("{}s", config.seconds),
        "--no-prompt".into(),
        "--target-stdout".into(),
        root.join("target.log").to_string_lossy().into_owned(),
        "--launch".into(),
        "--".into(),
        binary.to_string_lossy().into_owned(),
        "trace".into(),
        plan.to_string_lossy().into_owned(),
        "--output".into(),
        target.to_string_lossy().into_owned(),
        "--options".into(),
        options.to_string(),
    ];
    write_new(
        &root.join("command.json"),
        &json!(
            ["xcrun"]
                .into_iter()
                .chain(arguments.iter().map(String::as_str))
                .collect::<Vec<_>>()
        ),
    )?;
    let toc = root.join("toc.xml");
    let recorded = {
        let mut command = Command::new("xcrun");
        command.args(&arguments);
        invoke(
            &mut command,
            &root.join("record.log"),
            Duration::from_secs(config.seconds + 60),
        )
    };
    let exported = recorded.and_then(|()| export_toc(&source, &toc));
    let cleaned = cleanup(&toc, &binary);
    match (exported, cleaned) {
        (Ok(()), Ok(())) => {}
        (Err(error), _) | (Ok(()), Err(error)) => return Err(error),
    }
    let document = target.join("observation.json");
    if !document.exists() {
        return Err("target did not finish; partial recording retained".into());
    }
    for schema in [
        "os-signpost",
        "CountingModeSamples",
        "kdebug-counters-with-time-sample",
    ] {
        export(&source, schema, &root.join(format!("{schema}.xml")))?;
    }
    let summary = report::write(&document, &root.join("analysis"))?;
    let correlated = trace::correlate(&root)?;
    Ok(json!({
        "matched": correlated["matched"], "missing": correlated["missing"].as_array().map_or(0, Vec::len),
        "samples": correlated["samples"], "successful": summary["successful"], "complete": summary["complete"],
        "observation": document, "native": source, "analysis": root.join("analysis"),
    }))
}

pub(crate) fn prerequisite() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(
            "native recording requires macOS; spec trace and report remain portable".into(),
        );
    }
    let status = Command::new("xcrun")
        .args(["--find", "xctrace"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("xcrun is required for native recording: {error}"))?;
    if !status.success() {
        return Err(
            "xctrace is required for native recording; install Xcode command-line tools".into(),
        );
    }
    Ok(())
}

fn export_toc(source: &Path, toc: &Path) -> Result<()> {
    let file = File::create(toc).map_err(|error| error.to_string())?;
    let mut child = Command::new("xcrun")
        .args(["xctrace", "export", "--input"])
        .arg(source)
        .arg("--toc")
        .stdout(Stdio::from(file))
        .spawn()
        .map_err(|error| error.to_string())?;
    let status = match child
        .wait_timeout(Duration::from_secs(60))
        .map_err(|error| error.to_string())?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("xctrace table-of-contents export timed out".into());
        }
    };
    status
        .success()
        .then_some(())
        .ok_or_else(|| "xctrace table-of-contents export failed".into())
}

#[cfg(unix)]
fn cleanup(toc: &Path, binary: &Path) -> Result<()> {
    if !toc.exists() {
        return Ok(());
    }
    let source = std::fs::read_to_string(toc).map_err(|error| error.to_string())?;
    let document = roxmltree::Document::parse(&source).map_err(|error| error.to_string())?;
    let Some(process) = document.descendants().find(|node| {
        node.has_tag_name("process")
            && node
                .parent_element()
                .is_some_and(|parent| parent.has_tag_name("target"))
    }) else {
        return Ok(());
    };
    if process.attribute("return-exit-status").is_some() {
        return Ok(());
    }
    let pid = process
        .attribute("pid")
        .ok_or("xctrace target pid is absent")?
        .parse::<i32>()
        .map_err(|error| error.to_string())?;
    let output = Command::new("ps")
        .args(["-ww", "-p", &pid.to_string(), "-o", "command="])
        .output()
        .map_err(|error| error.to_string())?;
    let command = String::from_utf8_lossy(&output.stdout);
    if output.status.success()
        && command
            .trim_start()
            .starts_with(&format!("{} ", binary.display()))
        && let Err(error) = kill(Pid::from_raw(pid), Signal::SIGTERM)
        && error != nix::errno::Errno::ESRCH
    {
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(not(unix))]
fn cleanup(_toc: &Path, _binary: &Path) -> Result<()> {
    Err("native recording requires macOS".into())
}
