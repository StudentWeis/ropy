#!/usr/bin/env python3
"""Verify X11 popup reactivation with the pointer left inside the hidden window.

Run only on an isolated X11 desktop with one unpinned Ropy window already visible
and another application to receive focus. Requires xdotool and xwininfo. Uses the
configured activation shortcut (Ctrl+Shift+D by default); never forces focus or
maps the window directly. Saves native focus/map observations, including failure.
Example: DISPLAY=:98 python3 scripts/test_popup_focus.py --output /tmp/focus-check
"""

import argparse
import json
import os
import subprocess
import time
from pathlib import Path

from test_window_drag import geometry, xdo


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--hotkey", default="ctrl+shift+d")
    args = parser.parse_args()
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        parser.error("requires an isolated X11 DISPLAY with WAYLAND_DISPLAY unset")
    args.output.mkdir(parents=True, exist_ok=True)
    windows = xdo("search", "--onlyvisible", "--class", r"^com\.ropy\.app$").splitlines()
    if len(windows) != 1:
        raise RuntimeError(f"expected one visible Ropy window, got {windows}")
    window = windows[0]
    observations = []

    def wait_for_state(stage, visible):
        deadline = time.monotonic() + 5
        stable_since = None
        while time.monotonic() < deadline:
            mapped = "IsViewable" in subprocess.check_output(
                ["xwininfo", "-id", window], text=True
            )
            focus = xdo("getwindowfocus")
            observations.append({"stage": stage, "visible": mapped, "focus": focus})
            if mapped == visible and (focus == window) == visible:
                if stable_since is None:
                    stable_since = time.monotonic()
                # Include queued focus notifications after unmapping, not just
                # the first matching snapshot of the X server's state.
                if time.monotonic() - stable_since >= 0.3:
                    return
            else:
                stable_since = None
            time.sleep(0.05)
        raise AssertionError(f"{stage}: expected visible/focused={visible}, got {observations[-1]}")

    try:
        wait_for_state("initial", True)
        for cycle in range(3):
            for dismissal in ("Escape", args.hotkey):
                bounds = geometry(window)
                xdo("mousemove", "--sync", bounds["X"] + bounds["WIDTH"] // 2,
                    bounds["Y"] + bounds["HEIGHT"] // 2)
                wait_for_state(f"{cycle}-{dismissal}-pointer-inside", True)
                xdo("key", "--clearmodifiers", dismissal)
                wait_for_state(f"{cycle}-{dismissal}-hidden", False)
                xdo("key", "--clearmodifiers", args.hotkey)
                wait_for_state(f"{cycle}-{dismissal}-reopened", True)
    finally:
        (args.output / "observations.json").write_text(
            json.dumps({"display": os.environ["DISPLAY"], "window": window,
                        "observations": observations}, indent=2) + "\n"
        )
    print("PASS: pointer-inside Escape/hotkey dismissal and reactivation (3 cycles)", flush=True)


if __name__ == "__main__":
    main()
