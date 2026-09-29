import unittest
import json
import tempfile
from copy import deepcopy
from pathlib import Path
from unittest.mock import patch
import compare
from paired import resources
from reference import ACTIONS, freeze, identity, validate
from summary import duration, spread, topology, union


class Intervals(unittest.TestCase):
    def test_distribution(self):
        self.assertEqual(spread(list(range(1, 21))), {
            "samples": 20, "minimum": 1, "median": 10.5, "p95": 19, "maximum": 20,
        })

    def test_union(self):
        self.assertEqual(union([(0, 10), (2, 4), (8, 12), (20, 25)]), 17)
        self.assertEqual(union([]), 0)
        events = [{"pid": 1, "tid": tid, "ts": 0, "dur": 1000} for tid in [1, 1, 2]]
        self.assertEqual(duration(events), 2)

    def test_topology(self):
        events = [
            {"pid": 1, "tid": 1, "ph": "I", "name": "case:" + name, "ts": time}
            for name, time in [("start", 1000), ("script", 2000), ("end", 5000)]
        ]
        events.extend({"pid": 1, "tid": tid, "ph": "X", "name": name, "ts": time, "dur": size}
                      for tid, name, time, size in [
                          (1, "RunTask", 0, 3000), (1, "RunTask", 4000, 2000),
                          (1, "Layout", 2000, 500), (1, "Paint", 4000, 500),
                          (1, "Paint", 4100, 100), (2, "Layout", 2000, 1000),
                          (2, "RasterTask", 2500, 2000),
                      ])
        result = topology(events, "case")
        self.assertEqual(result["main"], 3)
        self.assertEqual(result["named"], 2)
        self.assertEqual(result["other"], 1)
        self.assertEqual(result["phases"]["Paint"], 0.5)
        self.assertEqual(result["raster"], 2)
        self.assertEqual(result["window"], 4)
        with self.assertRaises(KeyError):
            topology(events[1:], "case")


class References(unittest.TestCase):
    def test_duplicate_batches(self):
        with self.assertRaises(ValueError):
            freeze([Path("same"), Path("same")])

    def report(self):
        return {"config": {"runs": 2}, "errors": [], "runs": [
            {"mode": mode, "index": index, "stages": [
                {"action": action, "state": {"records": 384}} for action in ACTIONS
            ]} for mode in ("plain", "trace") for index in range(2)
        ]}

    def test_integrity(self):
        original = self.report()
        self.assertEqual(len(validate(original)), len(ACTIONS))
        for mutation in ("missing", "duplicate", "state", "action", "error", "mode"):
            report = deepcopy(original)
            if mutation == "missing":
                report["runs"].pop()
            elif mutation == "duplicate":
                report["runs"][-1]["index"] = 0
            elif mutation == "state":
                report["runs"][-1]["stages"][0]["state"]["records"] = 385
            elif mutation == "action":
                report["runs"][0]["stages"].reverse()
            elif mutation == "error":
                report["errors"] = ["script failed"]
            else:
                report["runs"].append({"mode": "warmup"})
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                validate(report)

    def test_identity(self):
        report = {key: "identity" for key in ("version", "browser", "host", "sources", "reset", "adaptations", "differences", "endpoint", "accounting", "gpu", "revision")}
        report["config"] = {"count": 384, "warmups": 3, "runs": 20, "url": "http://localhost:1234/"}
        report["gpu"] = {"auxAttributes": {"initializationTime": 0.1, "renderer": "Metal"}}
        changed = deepcopy(report)
        changed["config"]["url"] = "http://localhost:5678/"
        changed["gpu"]["auxAttributes"]["initializationTime"] = 0.2
        self.assertEqual(identity(report), identity(changed))
        changed["sources"] = "different harness"
        self.assertNotEqual(identity(report), identity(changed))
        del report["host"]
        with self.assertRaises(ValueError):
            identity(report)


class Captures(unittest.TestCase):
    def test_article_geometry(self):
        value = {"boxes": {"region": [24, 196, 680, 384], "record-0": [44, 216, 640, 27]}, "scroll": 48, "extent": 4096}
        changed = deepcopy(value)
        changed["boxes"]["record-0"][1] += 8
        changed["extent"] -= 512
        self.assertEqual(compare.projection("article", value), compare.projection("article", changed))
        changed["boxes"]["record-0"][3] *= 2
        self.assertNotEqual(compare.projection("article", value), compare.projection("article", changed))

    def test_gallery_geometry(self):
        native = {"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195]}, "scroll": 0, "extent": 24960}
        browser = {"boxes": {"region": [20, 148, 680, 320], "record-0": [20, 168, 680, 195.1875]}, "scroll": 0, "extent": 24984}
        self.assertEqual(compare.projection("gallery", native), compare.projection("gallery", browser))

    def test_resources(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            names = ("cards/main.js", "cards/style.css", "fonts/Lato-Regular.ttf", "benchmarks/normalize.css")
            for name in names:
                path = root / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(b"x")
            style = b"src:url(../fonts/Lato-Regular.ttf)"
            (root / "cards/style.css").write_bytes(style)
            result = resources(root, "cards", "normalized")
            self.assertEqual(set(result), set(names))
            self.assertEqual(result["cards/style.css"]["bytes"], len(style) + 11)
            self.assertEqual(result["cards/main.js"]["bytes"], 1)
            self.assertEqual(result["cards/main.js"]["fingerprint"], 0xAF63BD4C8601B7A7)

    def test_font_resources(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            names = ("article/main.js", "fonts/Regular.ttf", "fonts/Bold.ttf", "benchmarks/normalize.css")
            for name in names:
                path = root / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(b"x")
            style = root / "article/style.css"
            style.write_text("src:url('../fonts/Regular.ttf');src: url(../fonts/Bold.ttf)")
            result = resources(root, "article", "normalized")
            self.assertEqual(set(result), {*names, "article/style.css"})

    def test_requested_resources(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            names = ("gallery/main.js", "gallery/style.css", "pictures/dawn.png", "benchmarks/normalize.css")
            for name in names:
                path = root / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(b"x")
            result = resources(root, "gallery", "normalized", ["gallery/index.html", "pictures/dawn.png", "missing.png"])
            self.assertEqual(set(result), set(names))

    def reports(self):
        resources = {name: {"bytes": 1, "fingerprint": 2} for name in (
            "cards/main.js", "cards/style.css", "fonts/Lato-Regular.ttf", "benchmarks/normalize.css")}
        geometry = {"boxes": {"region": [20, 168, 680, 320]}, "scroll": 0, "extent": 640}
        native = {"debug": False, "case": {"id": "cards", "actions": ["edit"], "viewport": [720, 640]},
                  "count": 8, "edits": 1, "repetitions": 1, "capture": "native", "runs": [{
                      "failure": None, "closed": True,
                      "config": {"scenario.capture": True, "scenario.normalized": True},
                      "assets": {"app://scenarios/" + key: value for key, value in resources.items()},
                      "stages": [{"action": action, "records": 8, "geometry": geometry,
                                  "capture": {"file": "seed" if action == "steady" else action, "ready": 1000000}}
                                 for action in ("seed", "steady", "edit")]}]}
        browser = {"config": {"scenario": "cards", "count": 8, "runs": 1}, "errors": [],
                   "resources": resources, "endpoint": "browser", "runs": [{"index": 0, "stages": [
                       {"action": action, "file": "seed" if action == "steady" else action,
                        "oracle": "seed" if action == "steady" else action, "ready": 1,
                        "state": {"records": 8, "geometry": geometry}} for action in ("seed", "steady", "edit")]}]}
        return native, browser

    def test_integrity(self):
        native, browser = self.reports()
        compare.validate(native, browser)
        for mutation in ("assets", "profile", "actions", "lifecycle", "empty", "viewport"):
            changed = deepcopy(native)
            run = changed["runs"][0]
            if mutation == "assets":
                run["assets"]["app://scenarios/cards/main.js"]["fingerprint"] = 3
            elif mutation == "profile":
                run["config"]["scenario.normalized"] = False
            elif mutation == "actions":
                run["stages"].pop()
            elif mutation == "lifecycle":
                run["closed"] = False
            elif mutation == "empty":
                changed["repetitions"] = 0
                changed["runs"] = []
            else:
                changed["case"]["viewport"] = [640, 480]
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                compare.validate(changed, browser)
        browser["resources"].clear()
        with self.assertRaises(ValueError):
            compare.validate(native, browser)

    def test_freshness(self):
        for mutation in ("none", "stale", "oracle", "geometry"):
            native, browser = self.reports()
            stage = browser["runs"][0]["stages"][-1]
            if mutation == "stale":
                stage["file"] = stage["oracle"] = "seed"
            elif mutation == "oracle":
                stage["oracle"] = "seed"
            elif mutation == "geometry":
                stage["state"]["geometry"] = {"different": True}
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as root:
                root = Path(root)
                (root / "native.json").write_text(json.dumps(native))
                (root / "report.json").write_text(json.dumps(browser))
                with patch.object(compare, "pixels", side_effect=lambda path: path.name):
                    result = compare.compare(root / "native.json", root)
                self.assertEqual(result["passed"], mutation == "none")
                self.assertIsNone(result["ratio"])


if __name__ == "__main__":
    unittest.main()
