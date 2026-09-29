import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
from summary import summarize


ACTIONS = ("seed", "steady", "append", "edit", "reorder", "remove", "width", "scroll")


def identity(report):
    fields = ("version", "browser", "host", "sources", "reset", "adaptations", "differences", "endpoint", "accounting", "gpu")
    if any(not report.get(key) for key in fields):
        raise ValueError("reference requires complete browser, host, source and measurement identity")
    if not report.get("revision"):
        raise ValueError("reference requires source revision")
    result = {**{key: report[key] for key in fields},
              "workload": {key: report["config"][key] for key in ("count", "warmups")}}
    result["gpu"] = deepcopy(report["gpu"])
    result["gpu"].get("auxAttributes", {}).pop("initializationTime", None)
    return result


def validate(report):
    if report.get("errors") != []:
        raise ValueError("browser errors invalidate the reference")
    count = report["config"]["runs"]
    if count < 1:
        raise ValueError("reference requires measured repetitions")
    states = {}
    for mode in ("plain", "trace"):
        runs = [run for run in report["runs"] if run["mode"] == mode]
        if sorted(run["index"] for run in runs) != list(range(count)):
            raise ValueError("incomplete or duplicate repetitions")
        for run in runs:
            if tuple(stage["action"] for stage in run["stages"]) != ACTIONS:
                raise ValueError("incomplete or reordered actions")
            for stage in run["stages"]:
                action = stage["action"]
                if states.setdefault(action, stage["state"]) != stage["state"]:
                    raise ValueError("observed workload changed across repetitions")
    if len(report["runs"]) != count * 2:
        raise ValueError("unexpected measurement mode")
    return states


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def freeze(directories):
    if len({directory.resolve() for directory in directories}) != len(directories):
        raise ValueError("reference batches must be distinct")
    batches = []
    common = None
    observed = None
    for directory in directories:
        report = json.loads((directory / "report.json").read_text())
        signature = identity(report)
        states = validate(report)
        if common is not None and (common != signature or observed != states):
            raise ValueError("reference batches differ in environment, harness or observed workload")
        common, observed = signature, states
        summary = summarize(directory)
        batches.append({"id": directory.name, "revision": report["revision"],
                        "gpu_initialization_seconds": report["gpu"].get("auxAttributes", {}).get("initializationTime"),
                        "config": {key: report["config"][key] for key in ("count", "runs", "warmups")},
                        "artifacts": {name: digest(directory / name) for name in ("report.json", "trace.json", "summary.json")},
                        "actions": summary["actions"]})
    if len(batches) < 2:
        raise ValueError("reference requires at least two batches")
    actions = {}
    for action in ACTIONS:
        traces = [batch["actions"][action]["trace"] for batch in batches]
        actions[action] = {
            "main_median_range": [min(t["trace"]["main"]["median"] for t in traces), max(t["trace"]["main"]["median"] for t in traces)],
            "main_p95_range": [min(t["trace"]["main"]["p95"] for t in traces), max(t["trace"]["main"]["p95"] for t in traces)],
            "phase_median_ranges": {key: [min(t["phases"][key]["median"] for t in traces), max(t["phases"][key]["median"] for t in traces)] for key in traces[0]["phases"]},
        }
    return {"schema": 1, "status": "provisional", "scenario": "cards", "unit": "milliseconds",
            "analysis_sources": {name: digest(Path(__file__).with_name(name)) for name in ("reference.py", "summary.py")},
            "cross_engine_ratio": False, "identity": common, "states": observed,
            "actions": actions, "batches": [{key: value for key, value in batch.items() if key != "actions"} for batch in batches],
            "interpretation": {
                "primary": "Union of main-thread RunTask wall intervals within action markers; not thread CPU time or end-to-end frame completion.",
                "ranges": "Range of batch medians/p95 values, not a confidence interval or acceptance threshold.",
                "comparison": "Directional phase/work reference only; margin geometry and rendering endpoints remain unmatched to Plectron.",
                "secondary": "RasterTask/GPUTask thread intervals can overlap and are not hardware GPU duration; do not add them to main.",
                "plain": "Untraced CDP metrics expose instrumentation sensitivity but TaskDuration includes harness/protocol work.",
                "startup": "Not measured by this reference; fresh browser/application startup requires a separate endpoint contract.",
            }}


def main():
    parser = argparse.ArgumentParser(description="Freeze compatible Chromium observation batches into a provisional reference.")
    parser.add_argument("directories", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; choose a new reference")
    result = freeze(args.directories)
    with args.output.open("x") as output:
        output.write(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
