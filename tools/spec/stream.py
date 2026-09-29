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


def execute(binary, scenario, backend, rounds, lanes):
    process = subprocess.run(
        [str(binary), "stream", scenario, backend, str(rounds), str(lanes)],
        cwd=ROOT, capture_output=True, text=True,
    )
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"stream exited {process.returncode}")
    return json.loads(process.stdout)


def summarize(samples):
    groups = {}
    for sample in samples:
        key = f'{sample["backend"]}:{sample["lanes"]}'
        groups.setdefault(key, []).append(sample)
    return {
        key: {
            "processes": len(rows),
            "rounds": rows[0]["rounds"],
            "lanes": rows[0]["lanes"],
            "total": stats([row["total"] for row in rows]),
            "artifact": stats([
                row["total"] / (row["rounds"] * row["lanes"])
                for row in rows
            ]),
            "round": stats([value for row in rows for value in row["samples"]]),
        }
        for key, rows in groups.items()
    }


def main():
    parser = argparse.ArgumentParser(description="Measure warmed Metal compute streams.")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/spec")
    parser.add_argument("--scenario", default="chat")
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--rounds", type=int, default=20)
    parser.add_argument("--lanes", nargs="+", type=int, default=[1, 2, 4])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1 or not 1 <= args.rounds <= 1_000:
        parser.error("runs must be positive and rounds must be between 1 and 1000")
    if any(not 1 <= lanes <= 8 for lanes in args.lanes):
        parser.error("lanes must be between 1 and 8")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    write(output / "manifest.json", {
        "version": 1,
        "binary": str(binary),
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scenario": args.scenario,
        "runs": args.runs,
        "rounds": args.rounds,
        "lanes": args.lanes,
        "backends": ["metal", "executor"],
        "platform": platform.platform(),
        "process": "fresh process per sample; one unmeasured warmup",
        "endpoint": "compute completion; preparation, readback and presentation excluded",
    })
    samples = []
    for backend in ["metal", "executor"]:
        for lanes in args.lanes:
            for index in range(args.runs):
                print(f"{backend} lanes={lanes} {index + 1}/{args.runs}", flush=True)
                sample = execute(binary, args.scenario, backend, args.rounds, lanes)
                if sample["maximum"] > 1:
                    raise RuntimeError("CPU and Metal artifacts diverged")
                samples.append(sample)
                write(output / "samples.json", samples)
    write(output / "summary.json", {"unit": "nanoseconds", "summary": summarize(samples)})


if __name__ == "__main__":
    main()
