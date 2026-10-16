use crate::Scenario;
use crate::process::{TIMEOUT, capture, readiness, server, terminate, text};
use plectron_chromium::{Result, artifact, digest, paired, prerequisite, read, summary, write};
use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub struct Measurement {
    pub output: PathBuf,
    pub count: u64,
    pub runs: u64,
    pub warmups: u64,
    pub paired: bool,
    pub scenario: Scenario,
    pub host: Host,
}

pub struct Host {
    pub executable: PathBuf,
    pub architecture: &'static str,
}

struct Session<'a> {
    measurement: &'a Measurement,
    name: String,
    root: PathBuf,
}

impl Measurement {
    pub fn measure(&self) -> Result<()> {
        if self.count < 2 || self.runs < 1 {
            return Err("count >= 2 and runs >= 1 required".into());
        }
        if !matches!(self.scenario, Scenario::Cards) && !self.paired {
            return Err("additional scenarios require --paired".into());
        }
        let output = &self.output;
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::create_dir(output).map_err(|error| format!("{}: {error}", output.display()))?;
        prerequisite::record(
            output,
            capture(
                Command::new("playwright-cli")
                    .arg("--version")
                    .current_dir(output),
                TIMEOUT,
            ),
        )?;
        let root = plectron_chromium::repository()?;
        let log = File::create(output.join("serve.log")).map_err(|error| error.to_string())?;
        let mut server = Command::new(server(&root)?)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|error| format!("could not start plectron-serve: {error}"))?;
        let session = Session {
            measurement: self,
            name: format!("plectron-{}-{}", self.scenario.name(), std::process::id()),
            root,
        };
        let result = session.serve(&mut server);
        terminate(&mut server);
        result
    }
}

impl Session<'_> {
    fn serve(&self, server: &mut Child) -> Result<()> {
        let measurement = self.measurement;
        let scenario = measurement.scenario.name();
        let ready = readiness(server)?;
        let mut config = json!({
            "url": ready["url"],
            "output": measurement.output,
            "count": measurement.count,
            "runs": measurement.runs,
            "warmups": measurement.warmups,
            "scenario": scenario,
        });
        let apps = self.root.join("apps");
        let scripts = artifact::scripts(&apps, scenario)?;
        config["order"] = json!(artifact::entries(&apps, scenario)?);
        config["scripts"] = json!(
            scripts
                .iter()
                .map(|(name, artifact)| (name.clone(), artifact["body"].clone()))
                .collect::<Map<_, _>>()
        );
        if measurement.paired {
            self.pair(&mut config)?;
        }
        write(&measurement.output.join("config.json"), &config)?;
        let harness = self
            .root
            .join("tools/chromium")
            .join(if measurement.paired {
                "paired.js"
            } else {
                "cards.js"
            });
        let source = fs::read_to_string(&harness)
            .map_err(|error| format!("{}: {error}", harness.display()))?;
        let encoded = serde_json::to_string(&config).map_err(|error| error.to_string())?;
        let script = measurement.output.join("run.js");
        fs::write(&script, source.replace("__CONFIG__", &encoded))
            .map_err(|error| error.to_string())?;
        eprintln!("Starting Chromium {scenario} measurement");
        self.playwright(&["open".into(), "--browser=chrome".into()])?;
        let captured = self.capture(&script, &harness, &config, &scripts);
        let close = self.playwright(&["close".into()]).map(|_| ());
        captured.and(close)
    }

    fn pair(&self, config: &mut Value) -> Result<()> {
        let scenario = self.measurement.scenario.name();
        let stylesheet = self.root.join("apps/benchmarks/normalize.css");
        config["stylesheet"] = Value::String(
            fs::read_to_string(&stylesheet)
                .map_err(|error| format!("{}: {error}", stylesheet.display()))?,
        );
        let catalog = read(&self.root.join("apps/catalog.json"))?;
        let case = catalog
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["id"] == scenario))
            .ok_or_else(|| format!("scenario is absent from catalog: {scenario}"))?;
        for name in ["actions", "prefix", "region", "regions"] {
            if let Some(value) = case.get(name) {
                config[name] = value.clone();
            }
        }
        if config.get("regions").is_none() {
            config["regions"] = json!([case["region"]]);
        }
        Ok(())
    }

    fn capture(
        &self,
        script: &Path,
        harness: &Path,
        config: &Value,
        scripts: &std::collections::BTreeMap<String, Value>,
    ) -> Result<()> {
        let measurement = self.measurement;
        let output = &measurement.output;
        let root = &self.root;
        let result = self.playwright(&[
            "run-code".into(),
            format!("--filename={}", script.display()),
        ])?;
        let mut report: Value =
            serde_json::from_str(&result).map_err(|error| format!("browser report: {error}"))?;
        report["prerequisites"] = read(&output.join("prerequisites.json"))?;
        report["artifacts"] = artifact::verify(scripts, &report["scripts"])?;
        report["translator"] = json!({
            "sha256": digest(&fs::read(&measurement.host.executable).map_err(|error| error.to_string())?),
            "debug": cfg!(debug_assertions),
            "boundary": "Rust project translator; exact emitted Script bytes consumed in ordered browser workload scope",
        });
        report["worktree"] = Value::String(text("git", &["status", "--porcelain"], root)?);
        report
            .as_object_mut()
            .ok_or("browser report must be an object")?
            .remove("scripts");
        report["host"] = json!({
            "system": text("uname", &["-a"], root)?.trim(),
            "architecture": measurement.host.architecture,
        });
        report["revision"] =
            Value::String(text("git", &["rev-parse", "HEAD"], root)?.trim().into());
        let mut paths = vec![
            harness.to_path_buf(),
            root.join("apps")
                .join(measurement.scenario.name())
                .join("index.html"),
        ];
        paths.extend(self.inputs(&mut report, config)?);
        for script in scripts.values() {
            paths.extend(
                script["sources"]
                    .as_object()
                    .ok_or("missing script source identities")?
                    .keys()
                    .map(|name| root.join(name)),
            );
        }
        report["sources"] = identities(root, paths)?;
        if measurement.paired {
            paired::save(output, &mut report)?;
        }
        write(&output.join("report.json"), &report)?;
        if !measurement.paired {
            summary::summarize(output)?;
        }
        println!("{}", output.join("report.json").display());
        Ok(())
    }

    fn inputs(&self, report: &mut Value, config: &Value) -> Result<Vec<PathBuf>> {
        let apps = self.root.join("apps");
        let scenario = self.measurement.scenario.name();
        if !self.measurement.paired {
            let mut paths: Vec<_> = ["index.html", "style.css"]
                .map(|name| apps.join(scenario).join(name))
                .into();
            paths.push(apps.join("fonts/Lato-Regular.ttf"));
            return Ok(paths);
        }
        let requests: Vec<_> = report["requests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        report["resources"] = paired::resources(
            &apps,
            scenario,
            config["stylesheet"].as_str().unwrap_or_default(),
            &requests,
        )?;
        let artifacts = report["artifacts"]
            .as_object()
            .ok_or("missing script artifacts")?;
        for (name, artifact) in artifacts {
            if ["bytes", "fingerprint"]
                .into_iter()
                .any(|key| report["resources"][name][key] != artifact[key])
            {
                return Err(format!("browser script changed during run: {name}"));
            }
        }
        let mut paths: Vec<_> = report["resources"]
            .as_object()
            .into_iter()
            .flat_map(Map::keys)
            .map(|name| apps.join(name))
            .collect();
        paths.push(apps.join("catalog.json"));
        Ok(paths)
    }

    fn playwright(&self, args: &[String]) -> Result<String> {
        let output = &self.measurement.output;
        let result = capture(
            Command::new("playwright-cli")
                .arg(format!("-s={}", self.name))
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
}

fn identities(root: &Path, paths: Vec<PathBuf>) -> Result<Value> {
    paths
        .into_iter()
        .map(|path| {
            let data = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let name = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            Ok((name, json!(digest(&data))))
        })
        .collect::<Result<Map<_, _>>>()
        .map(Value::Object)
}
