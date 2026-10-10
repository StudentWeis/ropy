"""Capture isolated Ropy windows on macOS and compose the four original PNGs."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
THEMES = ("ropy-light", "ropy-dark", "nord-light", "everforest-night")


def stop_child(child):
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)


def wait_ready(child, ready, deadline):
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise RuntimeError("Capture process exited before rendering; see capture.log")
        if ready.exists() and ready.read_text() == str(child.pid):
            return
        time.sleep(0.1)
    raise TimeoutError("Ropy did not finish rendering within the capture timeout")


def capture_theme(binary, helper, theme, language, work, timeout=30):
    with tempfile.TemporaryDirectory(prefix=f"{theme}-", dir=work) as runtime:
        ready = Path(runtime) / "ready"
        target = work / f"{theme}.png"
        with (work / "capture.log").open("ab") as log:
            child = subprocess.Popen(
                [str(binary), "--promo", theme, language, str(ready)],
                stdout=log, stderr=subprocess.STDOUT,
            )
            try:
                deadline = time.monotonic() + timeout
                wait_ready(child, ready, deadline)
                previous = None
                stable = 0
                while time.monotonic() < deadline:
                    if child.poll() is not None:
                        raise RuntimeError("Capture process exited; see capture.log")
                    window_id = subprocess.check_output(
                        ["swift", str(helper), str(child.pid)], text=True, timeout=10
                    ).strip()
                    if window_id.isdecimal():
                        subprocess.run(
                            ["screencapture", "-x", "-o", f"-l{window_id}", str(target)],
                            check=True, timeout=5,
                        )
                        digest = hashlib.sha256(target.read_bytes()).digest()
                        stable = stable + 1 if digest == previous else 0
                        previous = digest
                        # Multiple captures after the renderer's readiness signal let
                        # the compositor settle without relying on a single sleep.
                        if stable >= 3:
                            return target
                    time.sleep(0.3)
                raise TimeoutError(f"No stable Ropy window for {theme}; check Screen Recording permission")
            finally:
                stop_child(child)


def build_binary():
    """Use Cargo's actual executable path, including configured target overrides."""
    build = subprocess.run(
        ["cargo", "build", "--locked", "-p", "ropy", "--bin", "ropy", "--message-format=json"],
        cwd=ROOT, stdout=subprocess.PIPE, text=True,
    )
    binary = None
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if message["reason"] == "compiler-message":
            print(message["message"].get("rendered", ""), file=sys.stderr, end="")
        if (message["reason"] == "compiler-artifact" and message["target"]["name"] == "ropy"
                and "bin" in message["target"]["kind"] and message.get("executable")):
            binary = Path(message["executable"])
    build.check_returncode()
    if binary is None:
        raise RuntimeError("Cargo did not report the capture executable")
    return binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target" / "promo")
    parser.add_argument("--language", default="en", choices=("en", "zh-CN", "ja"))
    parser.add_argument("--binary", type=Path, help="Use an already built Ropy binary")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("native screenshot capture currently requires macOS")
    helper = ROOT / "scripts" / "promo_window.swift"
    subprocess.run(["swift", str(helper), "--check-session"], check=True, timeout=15)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # A unique run directory preserves previous artifacts and excludes stale readiness files.
    run = Path(tempfile.mkdtemp(prefix="capture-", dir=output))
    binary = args.binary
    if binary is None:
        binary = build_binary()
    binary = binary.resolve(strict=True)
    images = [capture_theme(binary, helper, theme, args.language, run) for theme in THEMES]
    composite = run / "ropy-themes.png"
    subprocess.run([str(binary), "--compose-promo", str(composite), *map(str, images)], check=True)
    print(composite)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error
