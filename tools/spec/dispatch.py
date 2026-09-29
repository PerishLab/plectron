import argparse
import hashlib
import json
import platform
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def stats(values):
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def execute(binary, scenario, policy, delay):
    command = [str(binary), "dispatch", scenario, policy]
    if delay is not None:
        command.append(str(delay))
    process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"dispatch exited {process.returncode}")
    return json.loads(process.stdout)


def summarize(samples):
    groups = {}
    for sample in samples:
        key = f'{sample["policy"]}:{sample["delay"]}'
        groups.setdefault(key, []).append(sample)
    summary = {}
    for key, rows in groups.items():
        columns = {
            name: stats([row[name] for row in rows])
            for name in ["content", "preparation", "ready", "selected"]
        }
        for backend in ["metal", "cpu"]:
            attempts = [row[backend] for row in rows if row[backend] is not None]
            if attempts:
                columns[f"{backend}_start"] = stats([row["start"] for row in attempts])
                columns[f"{backend}_work"] = stats([row["work"] for row in attempts])
        summary[key] = {
            "samples": len(rows),
            "timing": columns,
            "winners": {
                backend: sum(row["winner"] == backend for row in rows)
                for backend in ["metal", "cpu"]
            },
            "duplicates": sum(row["duplicate"] for row in rows),
        }
    return summary


def main():
    parser = argparse.ArgumentParser(description="Measure fresh-process graphics dispatch policies.")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/spec")
    parser.add_argument("--scenario", default="chat")
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--delays", nargs="+", type=int, default=[0, 4, 8, 12, 16])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1 or any(delay < 0 for delay in args.delays):
        parser.error("runs must be positive and delays cannot be negative")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    policies = [("serial", None), ("cpu", None), ("overlap", None), ("executor", None)]
    policies.extend(("hedge", delay) for delay in args.delays)
    write(output / "manifest.json", {
        "version": 1,
        "binary": str(binary),
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scenario": args.scenario,
        "runs": args.runs,
        "policies": [{"name": name, "delay_ms": delay} for name, delay in policies],
        "platform": platform.platform(),
        "process": "fresh process per sample; OS file and driver caches uncontrolled",
        "endpoint": "first completed compute artifact; readback and presentation excluded",
    })
    samples = []
    for policy, delay in policies:
        for index in range(args.runs):
            print(f"{policy} {delay} {index + 1}/{args.runs}", flush=True)
            sample = execute(binary, args.scenario, policy, delay)
            if sample["maximum"] is not None and sample["maximum"] > 1:
                raise RuntimeError("CPU and Metal artifacts diverged")
            samples.append(sample)
            write(output / "samples.json", samples)
    write(output / "summary.json", {
        "unit": "nanoseconds",
        "summary": summarize(samples),
    })


if __name__ == "__main__":
    main()
