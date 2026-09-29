import base64
import hashlib
import json
import posixpath
import re
from collections import defaultdict
from summary import spread


def resources(root, scenario, stylesheet, requests=None):
    result = {}
    style = (root / scenario / "style.css").read_text()
    fonts = {
        posixpath.normpath(scenario + "/" + name)
        for _, name in re.findall(r"url\((['\"]?)([^)'\"]+)\1\)", style)
        if "://" not in name and not name.startswith("data:")
    }
    discovered = {
        name for name in requests or ()
        if name != scenario + "/index.html" and (root / name).is_file()
    }
    entry = scenario + "/main.js"
    scripts = {entry} if (root / entry).is_file() else set()
    names = sorted({scenario + "/style.css", *scripts, *fonts, *discovered, "benchmarks/normalize.css"})
    for name in names:
        data = (root / name).read_bytes()
        if name.endswith("/style.css"):
            data += b"\n" + stylesheet.encode()
        fingerprint = 14695981039346656037
        for byte in data:
            fingerprint = ((fingerprint * 1099511628211) ^ byte) & ((1 << 64) - 1)
        result[name] = {"bytes": len(data), "fingerprint": fingerprint}
    return result


def save(output, report):
    groups = defaultdict(list)
    for run in report["runs"]:
        for stage in run["stages"]:
            data = base64.b64decode(stage.pop("png"), validate=True)
            if data[:8] != b"\x89PNG\r\n\x1a\n":
                raise ValueError("capture did not return PNG")
            name = f'{run["index"]}-{stage["action"]}.png'
            (output / name).write_bytes(data)
            stage["file"] = name
            stage["sha256"] = hashlib.sha256(data).hexdigest()
            oracle = base64.b64decode(stage.pop("oracle"), validate=True)
            expected = f'{run["index"]}-{stage["action"]}-oracle.png'
            (output / expected).write_bytes(oracle)
            stage["oracle"] = expected
            groups[stage["action"]].append(stage)
    summary = {action: {key: spread([stage[key] for stage in stages]) for key in ("ready", "dispatch", "capture")}
               for action, stages in groups.items()}
    for action, stages in groups.items():
        summary[action]["work"] = {key: spread([stage["work"][key] for stage in stages]) for key in stages[0]["work"]}
    (output / "summary.json").write_text(json.dumps({"unit": "milliseconds", "actions": summary, "endpoint": report["endpoint"], "accounting": report["accounting"]}, indent=2) + "\n")
