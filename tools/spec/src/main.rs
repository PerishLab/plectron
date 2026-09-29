use clap::Parser;
use spec::{Profile, Result, budget, suite};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "spec", version = plumb::version!("PLECTRON_SPEC"))]
struct Cli {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    arguments: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("spec: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = Cli::parse().arguments;
    let text = args.last().is_some_and(|arg| arg == "--text");
    if text {
        args.pop();
        if !matches!(
            args.first().map(String::as_str),
            Some("summarize" | "compare" | "experiment")
        ) {
            return Err("--text is supported by summarize, compare and experiment".into());
        }
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        #[cfg(target_os = "macos")]
        ["dispatch", id, policy] => print(&spec::scenario::startup::dispatch(id, policy, 0)?),
        #[cfg(target_os = "macos")]
        ["dispatch", id, policy, delay] => print(&spec::scenario::startup::dispatch(
            id,
            policy,
            number("dispatch delay", delay)? as u64 * 1_000_000,
        )?),
        #[cfg(target_os = "macos")]
        ["startup", id] => {
            use std::io::Write;
            let report = spec::scenario::startup::run(id, |ready| {
                println!("{}", serde_json::to_string(ready).map_err(|e| e.to_string())?);
                std::io::stdout().flush().map_err(|e| e.to_string())
            })?;
            print(&report)
        }
        #[cfg(target_os = "macos")]
        ["vector"] => print(&spec::benchmark::vector::run()?),
        #[cfg(target_os = "macos")]
        ["layers"] => print(&spec::benchmark::layers::run()?),
        #[cfg(target_os = "macos")]
        ["strips"] => print(&spec::benchmark::strips::run()?),
        #[cfg(target_os = "macos")]
        ["motion"] => print(&spec::benchmark::strips::motion::run()?),
        #[cfg(target_os = "macos")]
        ["caret"] => print(&spec::benchmark::composition::caret::run((
            std::env::consts::ARCH,
            std::env::consts::OS,
        ))?),
        ["summarize", path] => {
            let report = spec::experiment::inspect(std::path::Path::new(path));
            outcome(&report, report.valid(), text.then(|| report.text()))
        }
        ["compare", before, after] => compare(before, after, None, text),
        ["compare", before, after, "--rules", rules] => compare(before, after, Some(rules), text),
        ["experiment", plan, "--output", output] => {
            let report = spec::experiment::run(
                std::path::Path::new(plan), std::path::Path::new(output),
                (std::env::consts::ARCH, std::env::consts::OS),
                std::env::current_exe().map_err(|e| e.to_string())?,
            )?;
            outcome(&report, report.valid(), text.then(|| report.text()))
        }
        ["trace", plan, "--output", output] => tracing(plan, output, "{}"),
        ["trace", plan, "--output", output, "--options", options] => tracing(plan, output, options),
        ["scenarios"] => print(&spec::scenario::load()?),
        ["scenario", id, mode] => scenario([id, mode, "small", "cpu"], "{}"),
        ["scenario", id, mode, size, backend] => scenario([id, mode, size, backend], "{}"),
        ["scenario", id, mode, size, backend, config] => scenario([id, mode, size, backend], config),
        ["channel"] => print(&spec::benchmark::channel::run(spec::benchmark::channel::Context {
            executable: std::env::current_exe().map_err(|error| error.to_string())?,
            temporary: std::env::temp_dir(),
            platform: format!("{} {}", std::env::consts::ARCH, std::env::consts::OS),
        })?),
        ["channel-child", root, mode, length, iterations] => print(&spec::benchmark::channel::child(root, mode, (length, iterations))?),
        ["composition"] => print(&spec::benchmark::composition::run((std::env::consts::ARCH, std::env::consts::OS))?),
        ["typography"] => print(&spec::typography::run()?),
        ["paint"] => print(&spec::benchmark::paint::run((
            std::env::consts::ARCH,
            std::env::consts::OS,
        ))?),
        ["scroll"] => print(&spec::benchmark::composition::scroll::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            &[32, 128, 512],
            256,
        )?),
        ["scroll", rows, iterations] => print(&spec::benchmark::composition::scroll::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            &[number("scroll rows", rows)?],
            number("scroll iterations", iterations)?,
        )?),
        ["mapping"] => print(&spec::benchmark::composition::mapping::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            2057,
            256,
        )?),
        ["mapping", spaces, iterations] => print(&spec::benchmark::composition::mapping::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            number("mapping spaces", spaces)?,
            number("mapping iterations", iterations)?,
        )?),
        ["visual"] => print(&spec::benchmark::composition::visual::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            128,
        )?),
        ["visual", iterations] => print(&spec::benchmark::composition::visual::run(
            (std::env::consts::ARCH, std::env::consts::OS),
            number("visual iterations", iterations)?,
        )?),
        ["layout"] => layout(None, None, None),
        ["layout", pages] => layout(Some(number("layout pages", pages)?), None, None),
        ["layout", pages, admission] => layout(
            Some(number("layout pages", pages)?),
            Some(number("layout admission", admission)?),
            None,
        ),
        ["layout", pages, admission, recurrence] => layout(
            Some(number("layout pages", pages)?),
            Some(number("layout admission", admission)?),
            Some(number("layout recurrence", recurrence)?),
        ),
        ["layout-compare", baseline, candidate] => layoutcompare(baseline, candidate),
        ["layout-cause", scenario, policy] => layoutcause(scenario, policy, false),
        ["layout-cause", scenario, policy, "--events"] => layoutcause(scenario, policy, true),
        ["benchmark"] => print(&spec::benchmark::run((
            std::env::consts::ARCH,
            std::env::consts::OS,
        ))?),
        ["budget"] => print(&budget::measure(&spec::root(), Profile::load()?)?),
        ["profile"] => print(Profile::load()?),
        ["matrix"] => print(&Profile::load()?.matrix()),
        [] | ["check"] => {
            let report = suite::run(Profile::load()?)?;
            print(&report)?;
            if report.totals[&suite::Status::Failed] != 0 {
                return Err("selected upstream checks failed".into());
            }
            Ok(())
        }
        #[cfg(target_os = "macos")]
        ["stream", id, backend] => print(&spec::scenario::startup::stream(id, backend, 20, 1)?),
        #[cfg(target_os = "macos")]
        ["stream", id, backend, rounds, lanes] => print(&spec::scenario::startup::stream(
            id,
            backend,
            number("stream rounds", rounds)?,
            number("stream lanes", lanes)?,
        )?),
        _ => Err(
            "usage: cargo spec trace PLAN --output DIRECTORY [--options JSON] | summarize REPORT/DIRECTORY [--text] | compare BEFORE AFTER [--rules FILE] [--text] | experiment PLAN --output DIRECTORY [--text] | dispatch ID serial/cpu/overlap/executor/hedge [DELAY_MS] | stream ID metal/executor [ROUNDS LANES] | startup ID | layout [PAGES [ADMISSION [RECURRENCE]]] | layout-compare BASELINE_JSON CANDIDATE_JSON | layout-cause SCENARIO POLICY_JSON [--events] | scroll [ROWS ITERATIONS] | mapping [SPACES ITERATIONS] | visual [ITERATIONS] | [check|budget|profile|matrix|benchmark|paint|typography|composition|channel|vector|layers|strips|motion|caret|scenarios|scenario ID --check/--bench [SIZE cpu/metal]]"
                .into(),
        ),
    }
}

fn print(value: &impl serde::Serialize) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn layout(pages: Option<usize>, admission: Option<usize>, recurrence: Option<usize>) -> Result<()> {
    let binary = std::env::current_exe().map_err(|error| error.to_string())?;
    print(&spec::benchmark::layout::run(
        (std::env::consts::ARCH, std::env::consts::OS),
        spec::benchmark::layout::Policy {
            pages,
            admission,
            recurrence,
            window: None,
        },
        &binary,
    )?)
}

fn layoutcompare(baseline: &str, candidate: &str) -> Result<()> {
    let parse =
        |value| serde_json::from_str(value).map_err(|error| format!("layout policy: {error}"));
    let baseline = parse(baseline)?;
    let candidate = parse(candidate)?;
    let binary = std::env::current_exe().map_err(|error| error.to_string())?;
    print(&spec::benchmark::layout::comparison::run(
        (std::env::consts::ARCH, std::env::consts::OS),
        [baseline, candidate],
        &binary,
    )?)
}

fn layoutcause(scenario: &str, policy: &str, events: bool) -> Result<()> {
    let policy = serde_json::from_str(policy).map_err(|error| format!("layout policy: {error}"))?;
    print(&spec::benchmark::layout::diagnosis::run(
        scenario, policy, events,
    )?)
}

fn number(name: &str, value: &str) -> Result<usize> {
    value.parse().map_err(|error| format!("{name}: {error}"))
}

fn scenario([id, mode, size, backend]: [&str; 4], settings: &str) -> Result<()> {
    let entries: config::Entries =
        serde_json::from_str(settings).map_err(|error| error.to_string())?;
    let config = config::Store::default();
    for (key, value) in entries {
        config.set(&key, value);
    }
    let report = spec::scenario::configured(
        id,
        spec::scenario::Options {
            size,
            mode,
            backend,
            platform: (std::env::consts::ARCH, std::env::consts::OS),
        },
        config,
    )?;
    print(&report)?;
    if report.runs.iter().any(|run| run.failure.is_some()) {
        return Err("scenario stopped; see structured failure in report".into());
    }
    Ok(())
}

fn outcome(report: &impl serde::Serialize, passed: bool, text: Option<String>) -> Result<()> {
    match text {
        Some(text) => print!("{text}"),
        None => print(report)?,
    }
    if passed {
        Ok(())
    } else {
        Err("experiment incomplete, incompatible or failed; see report".into())
    }
}
fn compare(before: &str, after: &str, rules: Option<&str>, text: bool) -> Result<()> {
    let rules = rules
        .map(|path| {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            serde_json::from_reader(file).map_err(|e| e.to_string())
        })
        .transpose()?
        .unwrap_or_default();
    let before = spec::experiment::read(std::path::Path::new(before))?;
    let after = spec::experiment::read(std::path::Path::new(after))?;
    let report = spec::experiment::compare(&before, &after, &rules);
    outcome(&report, report.passed, text.then(|| report.text()))
}

fn tracing(plan: &str, output: &str, options: &str) -> Result<()> {
    let options = serde_json::from_str(options).map_err(|e| format!("observe options: {e}"))?;
    let report = spec::experiment::observation::run(
        (std::path::Path::new(plan), std::path::Path::new(output)),
        (std::env::consts::ARCH, std::env::consts::OS),
        std::env::current_exe().map_err(|e| e.to_string())?,
        options,
    )?;
    outcome(&report, report.valid(), None)
}
