import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import subprocess
import sys
from summary import summarize


ROOT = Path(__file__).resolve().parents[2]
TIMEOUT = 120


def cli(session, args, output):
    result = subprocess.run(
        ["playwright-cli", "-s=" + session, "--raw", *args],
        cwd=output, capture_output=True, text=True, timeout=TIMEOUT,
    )
    (output / (args[0] + ".log")).write_text(result.stdout + result.stderr)
    if result.returncode or result.stdout.startswith("### Error"):
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description="Measure application workloads in local Chromium through playwright-cli.")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--count", type=int, default=384)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--paired", action="store_true")
    parser.add_argument("--scenario", choices=["article", "cards", "desk", "gallery", "isolation", "layers", "list"], default="cards")
    args = parser.parse_args()
    if args.count < 2 or args.runs < 1 or args.warmups < 0:
        parser.error("count >= 2, runs >= 1 and warmups >= 0 required")
    if args.scenario != "cards" and not args.paired:
        parser.error("additional scenarios require --paired")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    session = "plectron-" + args.scenario + "-" + str(os.getpid())
    with (output / "serve.log").open("w") as log:
        server = subprocess.Popen(
            [sys.executable, "-B", str(ROOT / "tools/serve/main.py")],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=log, text=True,
        )
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(server.stdout, selectors.EVENT_READ)
                if not selector.select(5):
                    raise RuntimeError("static server did not announce readiness")
            ready = json.loads(server.stdout.readline())
            config = {"url": ready["url"], "output": str(output), "count": args.count,
                      "runs": args.runs, "warmups": args.warmups, "scenario": args.scenario}
            if args.paired:
                config["stylesheet"] = (ROOT / "apps/benchmarks/normalize.css").read_text()
                catalog = json.loads((ROOT / "apps/catalog.json").read_text())
                case = next(case for case in catalog if case["id"] == args.scenario)
                config["actions"] = case["actions"]
                config["prefix"] = case["prefix"]
                config["region"] = case["region"]
                config["regions"] = case.get("regions", [case["region"]])
            (output / "config.json").write_text(json.dumps(config, indent=2) + "\n")
            harness = Path(__file__).with_name("paired.js" if args.paired else "cards.js")
            source = harness.read_text()
            script = output / "run.js"
            script.write_text(source.replace("__CONFIG__", json.dumps(config)))
            print("Starting Chromium " + args.scenario + " measurement", file=sys.stderr, flush=True)
            cli(session, ["open", "--browser=chrome"], output)
            try:
                result = cli(session, ["run-code", "--filename=" + str(script)], output)
                report = json.loads(result)
                report["host"] = {"system": platform.platform(), "architecture": platform.machine()}
                report["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
                paths = [harness]
                if args.paired:
                    from paired import resources, save
                    report["resources"] = resources(
                        ROOT / "apps", args.scenario, config["stylesheet"], report["requests"]
                    )
                    paths.extend(ROOT / "apps" / name for name in report["resources"])
                    paths.append(ROOT / "apps/catalog.json")
                else:
                    paths.extend(ROOT / "apps" / args.scenario / name for name in ("index.html", "main.js", "style.css"))
                    paths.append(ROOT / "apps/fonts/Lato-Regular.ttf")
                report["sources"] = {
                    str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in paths
                }
                if args.paired:
                    save(output, report)
                (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
                if not args.paired:
                    summarize(output)
                print(str(output / "report.json"))
            finally:
                cli(session, ["close"], output)
        finally:
            server.terminate()
            try:
                server.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.communicate()


if __name__ == "__main__":
    main()
