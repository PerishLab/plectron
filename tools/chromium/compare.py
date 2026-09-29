import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
from summary import spread


ROOT = Path(__file__).resolve().parents[2]


def pixels(path):
    from PIL import Image
    with Image.open(path) as image:
        if image.size != (720, 640):
            raise ValueError("capture extent mismatch")
        data = image.convert("RGB").tobytes()
        return hashlib.sha256(data).hexdigest()


def difference(before, after, path=""):
    if before == after:
        return []
    if isinstance(before, dict) and isinstance(after, dict):
        return [item for key in before.keys() | after.keys()
                for item in difference(before.get(key), after.get(key), path + "/" + key)]
    return [{"path": path, "plectron": before, "chromium": after}]


def projection(scenario, value):
    if scenario not in ("article", "gallery"):
        return value
    boxes = value["boxes"]
    def shape(box):
        result = [box[0], box[2], box[3]]
        return [round(item) for item in result] if scenario == "gallery" else result
    return {
        "region": [round(item) for item in boxes["region"]] if scenario == "gallery" else boxes["region"],
        "records": {
            name: shape(box)
            for name, box in boxes.items()
            if name.startswith("record-")
        },
        "scroll": value["scroll"],
    }


def validate(report, browser):
    if browser.get("errors"):
        raise ValueError("browser execution failed")
    if report["debug"]:
        raise ValueError("paired timing requires a release build")
    if report["case"]["id"] != browser["config"]["scenario"] or report["count"] != browser["config"]["count"]:
        raise ValueError("scenario workload mismatch")
    scenario = report["case"]["id"]
    resources = {scenario + "/style.css", "benchmarks/normalize.css"}
    if not resources.issubset(browser["resources"]):
        raise ValueError("incomplete resource identities")
    if report["case"]["viewport"] != [720, 640] or report["edits"] != 1:
        raise ValueError("unsupported viewport or edit workload")
    if report["repetitions"] < 1 or browser["config"]["runs"] < 1:
        raise ValueError("empty repetitions")
    if len(report["runs"]) != report["repetitions"] or len(browser["runs"]) != browser["config"]["runs"]:
        raise ValueError("incomplete repetitions")
    expected = ["seed", "steady", *report["case"]["actions"]]
    for run in report["runs"]:
        if run["failure"] or not run["closed"]:
            raise ValueError("native scenario failed or retained lifecycle")
        if not all(run["config"].get(key) is True for key in ("scenario.capture", "scenario.normalized")):
            raise ValueError("native profile must enable normalized capture")
        if [stage["action"] for stage in run["stages"]] != expected:
            raise ValueError("incomplete native actions")
        for name, value in browser["resources"].items():
            actual = run["assets"].get("app://scenarios/" + name, {})
            if any(actual.get(key) != value[key] for key in ("bytes", "fingerprint")):
                raise ValueError("resource mismatch: " + name)
    if sorted(run["index"] for run in browser["runs"]) != list(range(browser["config"]["runs"])):
        raise ValueError("duplicate or missing browser repetition")
    for run in browser["runs"]:
        if [stage["action"] for stage in run["stages"]] != expected:
            raise ValueError("incomplete browser actions")


def compare(native, directory):
    report = json.loads(native.read_text())
    browser = json.loads((directory / "report.json").read_text())
    validate(report, browser)
    groups = {name: defaultdict(list) for name in ("plectron", "chromium")}
    issues = []
    for name, runs in (("plectron", report["runs"]), ("chromium", browser["runs"])):
        known = {}
        for run in runs:
            previous = None
            for stage in run["stages"]:
                action = stage["action"]
                if name == "plectron":
                    capture = stage["capture"]
                    if not capture or not capture["file"]:
                        raise ValueError("native capture artifact missing")
                    file = native.parent / Path(capture["file"]).name
                    if not file.exists():
                        file = ROOT / capture["file"]
                    geometry, records, ready = stage["geometry"], stage["records"], capture["ready"] / 1e6
                    rendering = stage.get("rendering") or {}
                    work = {key: rendering.get(key, 0) / 1e6 for key in ("preparation", "completion", "readback")}
                    work.update({"content": (stage.get("evaluation", 0) + stage.get("publication", 0)) / 1e6,
                                 "encoding": capture.get("encoding", 0) / 1e6, "total": stage.get("total", 0) / 1e6})
                    work["cpu"] = work["content"] + work["preparation"]
                else:
                    file = directory / stage["file"]
                    geometry, records, ready = stage["state"]["geometry"], stage["state"]["records"], stage["ready"]
                    work = stage.get("work", {})
                digest = pixels(file)
                if name == "chromium" and digest != pixels(directory / stage["oracle"]):
                    issues.append({"engine": name, "action": action, "issue": "capture differs from settled oracle"})
                if action == "steady" and digest != previous:
                    issues.append({"engine": name, "action": action, "issue": "unchanged action changed pixels"})
                if previous == digest and action not in ("steady", "seed"):
                    issues.append({"engine": name, "action": action, "issue": "mutation did not change captured pixels"})
                if action in known and known[action] != (geometry, records, digest):
                    issues.append({"engine": name, "action": action, "issue": "workload or pixels changed between runs"})
                known[action] = geometry, records, digest
                previous = digest
                groups[name][action].append({"geometry": geometry, "records": records, "ready": ready, "work": work})
    actions = {}
    for action, values in groups["plectron"].items():
        other = groups["chromium"][action]
        before = {"geometry": projection(report["case"]["id"], values[0]["geometry"]), "records": values[0]["records"]}
        after = {"geometry": projection(report["case"]["id"], other[0]["geometry"]), "records": other[0]["records"]}
        differences = difference(before, after)
        variations = difference(values[0]["geometry"], other[0]["geometry"])
        actions[action] = {"geometry": not differences, "differences": differences,
                           "variations": variations,
                           "plectron": spread([value["ready"] for value in values]),
                           "chromium": spread([value["ready"] for value in other]),
                           "work": {name: {key: spread([value["work"][key] for value in stages]) for key in stages[0]["work"]}
                                    for name, stages in (("plectron", values), ("chromium", other))}}
    limits = ["Native invocation is in-process; Chromium includes CDP command transport and capture scheduling.",
              "PNG compression and renderer antialiasing differ; matching selected geometry is not pixel equivalence.",
              "Chromium Date.now has millisecond resolution; this does not measure physical presentation or pure engine execution."]
    if report["case"]["id"] in ("article", "gallery"):
        limits.append("Flow equivalence compares content geometry and scroll state; vertical offsets and extent retain Plectron's non-collapsing margin model as reported variations. Gallery content edges are compared at Plectron's integer-pixel geometry boundary.")
    return {"schema": 1, "status": "provisional", "passed": not issues and all(action["geometry"] for action in actions.values()),
            "analysis_sources": {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                                 for name in ("compare.py", "summary.py", "paired.py")},
            "classification": "directional capture API observation", "unit": "milliseconds", "actions": actions, "issues": issues,
            "endpoints": {"plectron": report["capture"], "chromium": browser["endpoint"]},
            "evidence": {"plectron": hashlib.sha256(native.read_bytes()).hexdigest(),
                         "chromium": hashlib.sha256((directory / "report.json").read_bytes()).hexdigest()},
            "identity": {"scenario": report["case"]["id"], "count": report["count"],
                         "native": {key: report.get(key) for key in ("backend", "architecture", "system", "rasterization", "debug")},
                         "config": {key: value for key, value in report["runs"][0]["config"].items() if key != "scenario.output"},
                         "browser": {key: browser.get(key) for key in ("version", "host", "sources", "adaptations", "accounting")},
                         "sampling": {key: browser["config"].get(key) for key in ("runs", "warmups")}},
            "ratio": None, "limits": limits}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Check paired workload, actual PNG freshness and capture observations.")
    parser.add_argument("native", type=Path)
    parser.add_argument("chromium", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists")
    result = compare(args.native, args.chromium)
    with args.output.open("x") as output:
        output.write(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"passed": result["passed"], "issues": result["issues"], "classification": result["classification"]}))
    raise SystemExit(0 if result["passed"] else 1)
