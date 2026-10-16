use super::{CASES, WARMUP, action, verify, wait};
use crate::application::Application;
use crate::bench::median;
use crate::{Result, absolute, array, digest, integer, ipc, read, report, write};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(clap::Args)]
pub struct Config {
    #[arg(long)]
    pub baseline: PathBuf,
    #[arg(long)]
    pub candidate: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub apps: PathBuf,
    #[arg(long, default_value_t = 3)]
    pub rounds: usize,
    #[arg(long, default_value_t = 20)]
    pub runs: usize,
    #[arg(long, default_value_t = 0.0)]
    pub interval: f64,
}

type Batch = BTreeMap<(String, bool, String), Value>;

#[derive(Clone, Copy)]
struct Variant {
    case: &'static str,
    action: &'static str,
    traced: bool,
    mode: &'static str,
}

struct Probe<'a> {
    config: &'a Config,
    root: PathBuf,
    apps: PathBuf,
    binaries: [(&'static str, PathBuf); 2],
}

pub fn run(config: &Config) -> Result<Value> {
    crate::application::require()?;
    let counted = config.runs >= 1 && config.rounds >= 1;
    let paced = config.interval.is_finite() && config.interval >= 0.0;
    if !(counted && paced) {
        return Err("runs and rounds must be positive; interval must be nonnegative".into());
    }
    let probe = Probe {
        config,
        root: crate::Output::prepare(&config.output)?,
        apps: absolute(&config.apps)?,
        binaries: [
            ("baseline", absolute(&config.baseline)?),
            ("candidate", absolute(&config.candidate)?),
        ],
    };
    let batches = (0..config.rounds)
        .map(|index| probe.round(index))
        .collect::<Result<Vec<_>>>()?;
    let comparison = compare(&batches)?;
    let identities: Map<_, _> = probe
        .binaries
        .iter()
        .map(|(name, path)| {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            Ok((
                (*name).into(),
                json!({"path": path, "sha256": digest(&bytes)}),
            ))
        })
        .collect::<Result<_>>()?;
    let result = json!({
        "unit": "milliseconds", "rounds": config.rounds, "runs": config.runs, "warmup": WARMUP,
        "client_interval_ms": config.interval, "binaries": identities,
        "endpoint": "Client eval connection through page response confirming completed content job and changed frame receipt: presented revision matches desired revision, displayed scene matches desired scene, no pending or issued frame",
        "limits": ["Metal completion and drawable-copy receipt is not physical scanout", "Full page inspection and JSON processing are included and timed separately; inspection can wake the main loop", "Small catalog scenes, six records each, 720x640 logical viewport", "No per-action hardware GPU duration exposed by this protocol", "Semantic scene/geometry hashes and work counters compared, not screenshots or pixel hashes", "Content/render/Metal algorithms are unchanged by the compared commits"],
        "comparison": comparison,
    });
    write(&probe.root.join("summary.json"), &result)?;
    Ok(result)
}

impl Probe<'_> {
    fn variants(&self) -> Vec<Variant> {
        let mut variants = Vec::new();
        for (case, action) in CASES {
            for traced in [false, true] {
                variants.extend(self.binaries.iter().map(|(mode, _)| Variant {
                    case,
                    action,
                    traced,
                    mode,
                }));
            }
        }
        variants
    }

    fn round(&self, index: usize) -> Result<Batch> {
        let variants = self.variants();
        let mut batch = Batch::new();
        for offset in 0..variants.len() {
            let variant = variants[(index + offset) % variants.len()];
            eprintln!(
                "[{}/{}] {} {} traced={}",
                index + 1,
                self.config.rounds,
                variant.case,
                variant.mode,
                variant.traced
            );
            let key = (variant.case.into(), variant.traced, variant.mode.into());
            batch.insert(key, self.capture(variant, index)?);
        }
        for variant in &variants {
            let reference = &batch[&(variant.case.into(), false, "baseline".into())];
            verify(
                reference,
                &batch[&(variant.case.into(), variant.traced, variant.mode.into())],
            )?;
        }
        Ok(batch)
    }

    fn capture(&self, variant: Variant, index: usize) -> Result<Value> {
        let binary = self
            .binaries
            .iter()
            .find(|(name, _)| *name == variant.mode)
            .map(|(_, path)| path.as_path())
            .ok_or("unknown frame binary")?;
        let directory = self.root.join(format!(
            "{index}-{}-{}-{}",
            variant.case,
            variant.mode,
            usize::from(variant.traced)
        ));
        let mut application = Application::start(binary, &directory, "metal", None)?;
        let captured = application
            .ready()
            .and_then(|()| self.observe(&application, variant, &directory));
        let closed = application.close();
        let result = match (captured, closed) {
            (Ok(value), Ok(())) => value,
            (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
        };
        write(&directory.join("result.json"), &result)?;
        Ok(result)
    }

    fn observe(
        &self,
        application: &Application,
        variant: Variant,
        directory: &Path,
    ) -> Result<Value> {
        let (expected, mut previous) = self.load(application, variant.case)?;
        let start = json!({"type": "start", "options": {"cpu": true}});
        let identity = if variant.traced {
            Some(integer(
                &application.observation(&start)?["session"],
                "session",
            )?)
        } else {
            None
        };
        let mut samples = Vec::new();
        for index in 0..self.config.runs + WARMUP {
            let (next, sample) = action(
                application,
                &previous,
                &format!("scenario.run(\"{}\", []); 1;", variant.action),
                Duration::from_secs_f64(self.config.interval / 1_000.0),
            )?;
            previous = next;
            if index >= WARMUP {
                samples.push(sample);
            }
        }
        write(&directory.join("snapshot.json"), &previous)?;
        if let Some(identity) = identity {
            let summary =
                report::write(&application.export(identity)?, &directory.join("analysis"))?;
            let complete = summary["complete"].as_bool() == Some(true)
                && summary["successful"].as_bool() == Some(true);
            complete
                .then_some(())
                .ok_or("incomplete frame observation")?;
        }
        let timing = ["wall", "admission", "inspection", "queries"]
            .into_iter()
            .map(|key| Ok((key.to_owned(), spread(&samples, &[key])?)))
            .collect::<Result<Map<_, _>>>()?;
        let computation = previous["computation"]
            .as_object()
            .ok_or("computation must be an object")?
            .keys()
            .map(|key| Ok((key.clone(), spread(&samples, &["computation", key])?)))
            .collect::<Result<Map<_, _>>>()?;
        Ok(
            json!({"case": variant.case, "action": variant.action, "count": expected, "traced": variant.traced, "samples": samples, "timing": timing, "computation": computation}),
        )
    }

    fn load(&self, application: &Application, case: &str) -> Result<(i64, Value)> {
        let root = serde_json::to_string(&self.apps.to_string_lossy())
            .map_err(|error| error.to_string())?;
        let source = format!(
            "const server = platform.serve({root}); server.scheme(\"app://frames/\"); const win = platform.window({{title: \"Frame receipt probe\", width: 720, height: 640}}); win.load(\"app://frames/{case}/index.html\"); 1;"
        );
        if application.evaluate(&source)?["result"] != "1" {
            return Err("launcher failed".into());
        }
        let (previous, _, _) = wait(application, None, None, Duration::ZERO)?;
        let catalog = read(&self.apps.join("catalog.json"))?;
        let expected = array(&catalog, "catalog")?
            .iter()
            .find(|item| item["id"] == case)
            .and_then(|item| item["sizes"]["small"].as_i64())
            .ok_or("case size is absent")?;
        let ids = serde_json::to_string(&(0..expected).collect::<Vec<_>>())
            .map_err(|error| error.to_string())?;
        let (previous, _) = action(
            application,
            &previous,
            &format!("scenario.seed({ids}); 1;"),
            Duration::ZERO,
        )?;
        Ok((expected, previous))
    }
}

fn spread(samples: &[Value], path: &[&str]) -> Result<Value> {
    let values: Vec<_> = samples
        .iter()
        .map(|sample| path.iter().fold(sample, |value, key| &value[*key]).clone())
        .collect();
    ipc::distribution(&values)
}

fn medians(batches: &[Batch], key: &(String, bool, String), metric: &str) -> Vec<f64> {
    batches
        .iter()
        .map(|batch| {
            batch[key]["timing"][metric]["median"]
                .as_f64()
                .unwrap_or(f64::NAN)
                / 1_000_000.0
        })
        .collect()
}

fn compare(batches: &[Batch]) -> Result<Value> {
    let keys = CASES.iter().flat_map(|(case, _)| {
        [false, true].into_iter().flat_map(move |traced| {
            ["wall", "admission", "inspection"].map(move |metric| (*case, traced, metric))
        })
    });
    let mut rows = Vec::new();
    for (case, traced, metric) in keys {
        let baseline = medians(batches, &(case.into(), traced, "baseline".into()), metric);
        let candidate = medians(batches, &(case.into(), traced, "candidate".into()), metric);
        let ratios = baseline
            .iter()
            .zip(&candidate)
            .map(|(base, value)| (value / base - 1.0) * 100.0)
            .collect::<Vec<_>>();
        rows.push(json!({"case": case, "traced": traced, "metric": metric, "baseline": baseline, "candidate": candidate, "paired_percent": median(&ratios)?}));
    }
    Ok(Value::Array(rows))
}
