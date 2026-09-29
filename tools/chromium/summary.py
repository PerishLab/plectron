import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import statistics


PHASES = ("UpdateLayoutTree", "Layout", "PrePaint", "Paint", "Layerize", "Commit")


def union(intervals):
    total = 0
    end = float("-inf")
    for start, stop in sorted(intervals):
        total += max(0, stop - max(start, end))
        end = max(end, stop)
    return total


def duration(events):
    threads = defaultdict(list)
    for event in events:
        threads[event["pid"], event["tid"]].append((event["ts"], event["ts"] + event["dur"]))
    return sum(union(intervals) for intervals in threads.values()) / 1000


def topology(events, marker):
    marks = {event["name"]: event for event in events if event["name"].startswith(marker + ":")}
    begin, script, end = (marks[marker + ":" + name] for name in ("start", "script", "end"))
    if not begin["ts"] <= script["ts"] <= end["ts"]:
        raise ValueError("unordered action markers")
    clipped = []
    for event in events:
        if event["ph"] != "X":
            continue
        start = max(event["ts"], begin["ts"])
        stop = min(event["ts"] + event["dur"], end["ts"])
        if start < stop:
            clipped.append({**event, "ts": start, "dur": stop - start})
    main = [event for event in clipped if (event["pid"], event["tid"]) == (begin["pid"], begin["tid"])]
    phases = {name: duration(event for event in main if event["name"] == name) for name in PHASES}
    script = {**begin, "dur": script["ts"] - begin["ts"]}
    named = duration([script] + [event for event in main if event["name"] in PHASES])
    active = duration(event for event in main if event["name"] == "RunTask")
    if active + 0.001 < named:
        raise ValueError("named phases are not covered by main-thread RunTask events")
    return {
        "main": active, "application": script["dur"] / 1000, "phases": phases,
        "named": named, "other": max(0, active - named),
        "window": (end["ts"] - begin["ts"]) / 1000,
        "raster": duration(event for event in clipped if event["name"] == "RasterTask"),
        "gpuTasks": duration(event for event in clipped if event["name"] == "GPUTask"),
    }


def spread(values):
    ordered = sorted(values)
    return {"samples": len(values), "minimum": ordered[0], "median": statistics.median(values),
            "p95": ordered[math.ceil(len(ordered) * 0.95) - 1], "maximum": ordered[-1]}


def summarize(directory):
    report = json.loads((directory / "report.json").read_text())
    events = json.loads((directory / "trace.json").read_text())["traceEvents"]
    groups = defaultdict(list)
    for run in report["runs"]:
        for stage in run["stages"]:
            if run["mode"] == "trace":
                stage["trace"] = topology(events, stage["marker"])
            groups[run["mode"], stage["action"]].append(stage)
    actions = defaultdict(dict)
    for (mode, action), stages in groups.items():
        result = {
            "metrics": {name: spread([stage["timing"][name] for stage in stages]) for name in stages[0]["timing"]},
            "layouts": spread([stage["layouts"] for stage in stages]),
            "styles": spread([stage["styles"] for stage in stages]),
        }
        if mode == "trace":
            result["trace"] = {
                name: spread([stage["trace"][name] for stage in stages])
                for name in ("main", "application", "named", "other", "window", "raster", "gpuTasks")
            }
            result["phases"] = {
                name: spread([stage["trace"]["phases"][name] for stage in stages]) for name in PHASES
            }
            result["representative"] = sorted(stages, key=lambda stage: stage["trace"]["main"])[len(stages) // 2]["trace"]
        actions[action][mode] = result
    summary = {
        "unit": "milliseconds", "version": report["version"], "actions": actions,
        "accounting": "Trace main = union of RunTask intervals on the marker thread, clipped to the action window. Named phases are inclusive; named unions prevent double counting. Raster and gpuTasks sum per-thread wall intervals, can overlap main, and are not GPU execution time. Independent medians are not additive.",
    }
    (directory / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (directory / "stages.json").write_text(json.dumps(report["runs"], indent=2) + "\n")
    return summary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Summarize a Chromium cards report and native trace.")
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(args.directory), indent=2))
