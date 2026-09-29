import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import statistics
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[2]
TIMEOUT = 60


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def result(payload):
    value = payload["result"]
    if "Err" in value:
        raise RuntimeError(value["Err"])
    return value["Ok"]


def control(binary, env, pid, *arguments, check=True):
    completed = subprocess.run(
        [str(binary), "ctl", str(pid), *arguments], cwd=ROOT, env=env,
        capture_output=True, text=True, timeout=10,
    )
    if completed.returncode:
        if check:
            raise RuntimeError(completed.stderr.strip() or "control request failed")
        return None
    return result(json.loads(completed.stdout))


def lifecycle(record):
    produced = record["produced"]
    if produced is None:
        raise RuntimeError("first frame completed without a production timestamp")
    admitted, detected = record["admitted"], record["detected"]
    visible = record.get("visible")
    seen = record.get("seen")
    if not admitted <= produced <= detected:
        raise RuntimeError("first-frame timestamps are not monotonic")
    if visible is not None and not produced <= visible <= seen:
        raise RuntimeError("first-frame visibility timestamps are not monotonic")
    return {
        "submitted": admitted,
        "backend_completed": produced,
        "visible": visible,
        "seen": seen,
        "drops": record["drops"],
        "interactive": detected,
        "submit_to_backend": produced - admitted,
        "backend_to_interactive": detected - produced,
        "backend_to_visible": None if visible is None else visible - produced,
        "visible_to_detection": None if visible is None else seen - visible,
    }


def first(binary, env, process, started, visibility_wait):
    command = json.dumps({"type": "read", "record": 1}, separators=(",", ":"))
    deadline = time.monotonic() + TIMEOUT
    visibility_deadline = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"application exited {process.returncode} before its first frame")
        reply = control(binary, env, process.pid, "query", command, check=False)
        if reply is not None:
            status = reply["reply"]["status"]
            if status["type"] == "record":
                record = status["record"]
                if record["state"]["type"] == "complete":
                    if "Err" in record["state"]["result"]:
                        raise RuntimeError(record["state"]["result"]["Err"])
                    target = record["target"]
                    if target["type"] != "frame" or target["unit"] != 1:
                        raise RuntimeError("record 1 is not the first content frame")
                    if visibility_wait and record.get("visible") is None:
                        if visibility_deadline is None:
                            visibility_deadline = time.monotonic() + visibility_wait
                        if time.monotonic() < visibility_deadline:
                            time.sleep(0.005)
                            continue
                    return record, time.perf_counter_ns() - started
                if record["state"]["type"] != "pending":
                    raise RuntimeError(f"first frame ended as {record['state']['type']}")
            elif status["type"] == "evicted":
                raise RuntimeError("first-frame record was evicted before observation")
        time.sleep(0.005)
    raise RuntimeError("application did not complete its first frame")


def application(binary, root, backend, output, index, visibility_wait):
    stdout = output / f"{backend}-{index}.stdout"
    stderr = output / f"{backend}-{index}.stderr"
    with tempfile.TemporaryDirectory(prefix="plectron-startup-") as directory:
        env = os.environ.copy()
        env["PLECTRON_CONTROL_DIR"] = directory
        env["PLECTRON_RENDERER"] = backend
        with stdout.open("w") as out, stderr.open("w") as error:
            started = time.perf_counter_ns()
            process = subprocess.Popen(
                [str(binary), "dev", "start", str(root)], cwd=ROOT, env=env,
                stdout=out, stderr=error, text=True,
            )
            try:
                wait = visibility_wait if backend == "metal" else 0
                record, observed = first(binary, env, process, started, wait)
                page = control(binary, env, process.pid, "page", "1")
                sample = {
                    "pid": process.pid,
                    "backend": backend,
                    "observed": observed,
                    "frame": record,
                    "lifecycle": lifecycle(record),
                    "milestones": page["snapshot"]["milestones"],
                }
                control(binary, env, process.pid, "shutdown")
                process.wait(timeout=TIMEOUT)
                if process.returncode:
                    raise RuntimeError(f"application exited {process.returncode}; see {stderr.name}")
                return sample
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


def scenario(binary, name, output, index):
    with (output / f"{index}.stderr").open("w") as log:
        start = time.perf_counter_ns()
        process = subprocess.Popen(
            [str(binary), "startup", name], stdout=subprocess.PIPE,
            stderr=log, text=True, cwd=ROOT,
        )
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                if not selector.select(TIMEOUT):
                    raise RuntimeError("startup did not produce a first-frame signal")
            line = process.stdout.readline()
            ready = time.perf_counter_ns() - start
            remainder, _ = process.communicate(timeout=TIMEOUT)
            (output / f"{index}.stdout").write_text(line + remainder)
            if process.returncode:
                raise RuntimeError(f"startup exited {process.returncode}; see {index}.stderr")
            return {
                "pid": process.pid, "ready": ready,
                "first": json.loads(line), "checked": json.loads(remainder),
            }
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


def stats(columns):
    return {
        key: {"minimum": min(values), "median": statistics.median(values), "maximum": max(values)}
        for key, values in columns.items()
    }


def applications(args, output):
    binary, root = args.plectron.resolve(), args.application.resolve()
    write(output / "manifest.json", {
        "mode": "application", "binary": str(binary),
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "application": str(root), "backends": args.backends,
        "runs": args.runs, "platform": platform.platform(),
        "native_window": True, "focus": "incidental and unobserved",
        "visibility_wait_seconds": args.visibility_wait,
    })
    samples = []
    for backend in args.backends:
        for index in range(args.runs):
            print(f"{backend} startup {index + 1}/{args.runs}", flush=True)
            samples.append(application(binary, root, backend, output, index, args.visibility_wait))
            write(output / "samples.json", samples)
    summaries = {}
    for backend in args.backends:
        selected = [sample for sample in samples if sample["backend"] == backend]
        columns = {
            key: [sample["lifecycle"][key] for sample in selected]
            for key in ["submitted", "backend_completed", "interactive", "submit_to_backend", "backend_to_interactive", "drops"]
        }
        visible = [sample for sample in selected if sample["lifecycle"]["visible"] is not None]
        if visible:
            for key in ["visible", "seen", "backend_to_visible", "visible_to_detection"]:
                columns[key] = [sample["lifecycle"][key] for sample in visible]
        columns["observed"] = [sample["observed"] for sample in selected]
        names = ["loaded", "runtime", "activation", "scene", "publication", "frame", "presentation"]
        for name in names:
            values = [sample["milestones"][name] for sample in selected]
            if any(value is None for value in values):
                raise RuntimeError(f"first frame is missing the {name} milestone")
            columns[name] = values
        for start, end in zip(names, names[1:]):
            columns[f"{start}_to_{end}"] = [
                sample["milestones"][end] - sample["milestones"][start]
                for sample in selected
            ]
        summaries[backend] = stats(columns)
        summaries[backend]["visibility_coverage"] = {
            "observed": len(visible), "total": len(selected),
        }
    write(output / "summary.json", {
        "unit": "nanoseconds", "samples_per_backend": args.runs,
        "summary": summaries,
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "clock": "submitted, backend_completed, visible and interactive share the platform lifetime monotonic clock",
        "milestones": "loaded through presentation share the page-relative monotonic clock",
        "endpoint": {
            "submitted": "first frame admitted immediately before backend rendering",
            "backend_completed": "backend completion timestamp; Metal includes its presentation copy",
            "visible": "physical drawable presentation when the platform supplies evidence; absent otherwise",
            "interactive": "backend completion consumed and published to the page; logical input can be accepted independently of visibility",
            "observed": "parent-process detection through passive IPC; polling makes this an upper bound",
        },
    })


def scenarios(args, output):
    binary = args.binary.resolve()
    write(output / "manifest.json", {
        "mode": "scenario", "binary": str(binary),
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scenario": args.scenario, "runs": args.runs, "platform": platform.platform(),
        "native_window": False, "focus": "absent",
    })
    samples = []
    for index in range(args.runs):
        print(f"Startup {index + 1}/{args.runs}", flush=True)
        samples.append(scenario(binary, args.scenario, output, index))
        write(output / "samples.json", samples)
    if len({sample["checked"]["checksum"] for sample in samples}) != 1:
        raise RuntimeError("startup output changed across fresh processes")
    if any(sample["checked"]["assets"] != samples[0]["checked"]["assets"] for sample in samples):
        raise RuntimeError("startup assets changed across fresh processes")
    columns = {
        key: [sample["first"][key] for sample in samples]
        for key in ["device", "content", "preparation", "completion", "first"]
    }
    for key in ["loaded", "runtime", "activation", "scene", "publication", "frame", "presentation"]:
        columns[key] = [sample["first"]["milestones"][key] for sample in samples]
    columns["ready"] = [sample["ready"] for sample in samples]
    for key in ["driver", "library", "states"]:
        columns[key] = [sample["first"]["setup"][key] for sample in samples]
    write(output / "summary.json", {
        "unit": "nanoseconds", "samples": args.runs, "summary": stats(columns),
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "endpoint": "ready includes spawn/loader and delivery of first GPU-completed signal; CPU verification/readback follows signal",
        "milestones": "page-relative; presentation is the first successful frame acknowledgement after GPU completion",
        "provider": "app://scenarios mapped to local files; no HTTP network roundtrip",
    })


def main():
    parser = argparse.ArgumentParser(description="Measure fresh-process Plectron first-frame completion.")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/spec")
    parser.add_argument("--scenario", default="cards")
    parser.add_argument("--application", type=Path)
    parser.add_argument("--native-window", action="store_true")
    parser.add_argument("--plectron", type=Path, default=ROOT / "target/release/plectron")
    parser.add_argument("--backends", nargs="+", choices=["cpu", "metal"], default=["cpu", "metal"])
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--visibility-wait", type=float, default=2.0)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("runs must be positive")
    if args.visibility_wait < 0:
        parser.error("visibility wait cannot be negative")
    if args.application and not args.native_window:
        parser.error("--application opens native windows and requires --native-window")
    if args.native_window and not args.application:
        parser.error("--native-window requires --application")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    if args.application:
        applications(args, output)
    else:
        scenarios(args, output)


if __name__ == "__main__":
    main()
