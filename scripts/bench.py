#!/usr/bin/env python3
"""Small, informational release benchmark runner (Python 3.11+)."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu",
           "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc")


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def report(current, baseline=None):
    lines = [f"# Ropy {current['version']} benchmark", "",
             f"Commit: `{current['commit']}`; dirty: {current['dirty']}", "",
             "Informational only. Lower is better. RSS is an isolated hidden desktop,",
             "without clipboard monitoring, tray, hotkeys, updater or autostart.",
             f"Memory: {current.get('memory_status', 'not measured')}; package: {'measured' if 'package_bytes' in current['metrics'] else 'not measured'}.", "",
             "| Metric | Previous | Current | Change |", "|---|---:|---:|---:|"]
    compatible = baseline is not None and all(
        current.get(key) == baseline.get(key)
        for key in ("schema_version", "suite_version", "fixture_version", "environment"))
    before = baseline.get("metrics", {}) if baseline else {}
    for name in sorted(current["metrics"].keys() | before.keys()):
        old, new = before.get(name), current["metrics"].get(name)
        def display(metric):
            if not metric:
                return "not measured"
            scale, unit = 1, metric["unit"]
            if unit == "bytes":
                scale, unit = 1024 ** 2, "MiB"
            elif unit == "ns" and metric["value"] >= 1_000_000:
                scale, unit = 1_000_000, "ms"
            elif unit == "ns" and metric["value"] >= 1000:
                scale, unit = 1000, "µs"
            value = f"{metric['value'] / scale:.2f} {unit}"
            if "confidence_interval" in metric:
                low, high = metric["confidence_interval"]
                value += f" (95% CI {low / scale:.2f}–{high / scale:.2f} {unit})"
            return value
        delta = "—"
        if baseline:
            delta = "not comparable"
            if (compatible and old and new and old["unit"] == new["unit"]
                    and old.get("context") == new.get("context") and old["value"] > 0):
                delta = f"{(new['value'] / old['value'] - 1) * 100:+.1f}%"
        lines.append(f"| {name} | {display(old)} | {display(new)} | {delta} |")
    lines += ["", "Environment:", "", "```json", json.dumps(current["environment"], indent=2), "```", ""]
    return "\n".join(lines)


def release_sizes(directory, version):
    artifacts = []
    for path in sorted(directory.iterdir()):
        target = next((target for target in TARGETS if target in path.name), None)
        if not target or not path.name.endswith((".tar.xz", ".tar.gz", ".zip", ".dmg")):
            continue
        item = {"name": path.name, "target": target, "package_bytes": path.stat().st_size,
                "sha256": sha256(path)}
        members = []
        if path.name.endswith(".zip"):
            with zipfile.ZipFile(path) as archive:
                members = [(entry.filename, entry.file_size) for entry in archive.infolist() if not entry.is_dir()]
        elif ".tar." in path.name:
            with tarfile.open(path) as archive:
                members = [(entry.name, entry.size) for entry in archive if entry.isfile()]
        binaries = [size for name, size in members if Path(name).name in ("ropy", "ropy.exe")]
        if len(binaries) == 1:
            item["binary_bytes"] = binaries[0]
        artifacts.append(item)
    if not artifacts:
        raise ValueError("no recognized release packages found")
    return {"schema_version": 1, "version": version, "artifacts": artifacts}


def memory_samples(binary):
    if platform.system() != "Darwin":
        raise ValueError("RSS collection currently requires macOS; use --skip-memory elsewhere")
    with tempfile.TemporaryDirectory(prefix="ropy-bench-") as directory:
        ready = Path(directory) / "ready.json"
        with (Path(directory) / "desktop.log").open("w+") as log:
            process = subprocess.Popen([str(binary), "--bench-memory", str(ready)], stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 60
                while not ready.exists():
                    if process.poll() is not None or time.monotonic() > deadline:
                        log.seek(0)
                        raise RuntimeError("benchmark desktop failed to become ready:\n" + log.read())
                    time.sleep(0.1)
                state = json.loads(ready.read_text())
                if state != {"stored_records": 200, "loaded_records": 100}:
                    raise RuntimeError(f"unexpected desktop fixture: {state}")
                time.sleep(10)
                samples = []
                for _ in range(30):
                    if process.poll() is not None:
                        raise RuntimeError("benchmark desktop exited during sampling")
                    rss = int(output("ps", "-o", "rss=", "-p", str(process.pid))) * 1024
                    if rss <= 0:
                        raise RuntimeError("invalid RSS sample")
                    samples.append(rss)
                    time.sleep(1)
                return {"value": statistics.median(samples), "unit": "bytes", "samples": samples}
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


def build_binary():
    # Cargo chooses target directories from config/environment. Read its actual
    # executable path rather than risking a stale target/release binary.
    build = subprocess.run(["cargo", "build", "--locked", "--release", "-p", "ropy",
                            "--message-format=json"], cwd=ROOT, stdout=subprocess.PIPE, text=True)
    binary = None
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if message["reason"] == "compiler-message":
            print(message["message"].get("rendered", ""), file=sys.stderr, end="")
        if message["reason"] == "compiler-artifact" and message["target"]["name"] == "ropy" and message.get("executable"):
            binary = Path(message["executable"])
    build.check_returncode()
    if binary is None:
        raise RuntimeError("Cargo did not report the release executable")
    return binary


def run(args):
    if platform.system() != "Darwin" and not args.skip_memory:
        raise ValueError("RSS requires macOS; use --skip-memory for core-only measurement")
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = manifest["package"]["version"]
    destination = (args.output or ROOT / "target" / "bench" / version).resolve()
    # Preserve previous samples rather than silently overwriting a release baseline.
    destination.mkdir(parents=True, exist_ok=False)
    environment = {"os": platform.system(), "os_version": platform.release(),
                   "arch": platform.machine(), "machine": platform.node(),
                   "cpu": output("sysctl", "-n", "machdep.cpu.brand_string") if platform.system() == "Darwin" else platform.processor(),
                   "rustc": output("rustc", "-Vv"), "profile": manifest["profile"],
                   "build_overrides": {key: value for key, value in os.environ.items()
                                       if key.startswith(("CARGO_PROFILE_", "CARGO_TARGET_"))
                                       or key in ("CARGO_BUILD_TARGET", "RUSTC", "RUSTC_WRAPPER")},
                   "rustflags": os.environ.get("CARGO_ENCODED_RUSTFLAGS", os.environ.get("RUSTFLAGS", ""))}
    result = {"schema_version": 1, "suite_version": 1, "fixture_version": 1,
              "collected_at": datetime.now(timezone.utc).isoformat(),
              "version": version, "commit": output("git", "rev-parse", "HEAD"),
              "dirty": bool(output("git", "status", "--porcelain")),
              "environment": environment, "metrics": {}}
    binary = build_binary()
    result["binary_sha256"] = sha256(binary)
    result["metrics"]["binary_bytes"] = {"value": binary.stat().st_size, "unit": "bytes"}
    criterion_dir = destination / "criterion"
    env = dict(os.environ, CRITERION_HOME=str(criterion_dir))
    subprocess.run(["cargo", "bench", "--locked", "-p", "ropy-core", "--bench", "repository", "--", "--noplot"],
                   cwd=ROOT, env=env, check=True)
    for name in ("insert", "dedup", "read_100"):
        estimates = json.loads((criterion_dir / name / "new" / "estimates.json").read_text())
        median = estimates["median"]
        interval = median["confidence_interval"]
        result["metrics"][name + "_ns"] = {
            "value": median["point_estimate"], "unit": "ns",
            "confidence_interval": [interval["lower_bound"], interval["upper_bound"]]}
    if not args.skip_memory:
        result["metrics"]["idle_rss_bytes"] = memory_samples(binary)
    result["memory_status"] = "skipped" if args.skip_memory else "measured"
    if args.package:
        result["package"] = {"name": args.package.name, "sha256": sha256(args.package)}
        result["metrics"]["package_bytes"] = {"value": args.package.stat().st_size, "unit": "bytes",
                                               "context": {"format": "".join(args.package.suffixes),
                                                           "target": next((target for target in TARGETS if target in args.package.name), "unknown"),
                                                           "app_bundle": "-app." in args.package.name}}
    (destination / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    (destination / "report.md").write_text(report(result))
    print(destination / "report.md")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("run")
    collect.add_argument("--output", type=Path)
    collect.add_argument("--skip-memory", action="store_true")
    collect.add_argument("--package", type=Path, help="optional matching release download")
    compare = commands.add_parser("compare")
    compare.add_argument("--baseline", type=Path, required=True)
    compare.add_argument("--current", type=Path)
    sizes = commands.add_parser("sizes")
    sizes.add_argument("--directory", type=Path, required=True)
    sizes.add_argument("--version", required=True)
    sizes.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "run":
        run(args)
    elif args.command == "compare":
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
        current = args.current or ROOT / "target" / "bench" / version / "result.json"
        print(report(json.loads(current.read_text()), json.loads(args.baseline.read_text())))
    else:
        args.output.write_text(json.dumps(release_sizes(args.directory, args.version), indent=2) + "\n")


if __name__ == "__main__":
    main()
