#!/usr/bin/env python3
"""Verify native dragging on an isolated X11 desktop with Ropy already visible.

Requires xdotool, xwininfo and PyGObject/Gdk 3. Run with DISPLAY pointing to the test
desktop, never the user's desktop. Coordinates assume the default 400px window
and must be checked against the saved before screenshot when the layout changes.
Example: DISPLAY=:98 python3 scripts/test_window_drag.py --output /tmp/drag-check
"""

import argparse
import json
import os
import re
import subprocess
import time
from pathlib import Path


def xdo(*args):
    return subprocess.check_output(["xdotool", *map(str, args)], text=True).strip()


def geometry(window):
    # xdotool can double-count frame offsets for reparented client windows.
    info = subprocess.check_output(["xwininfo", "-id", window], text=True)
    fields = {"X": "Absolute upper-left X", "Y": "Absolute upper-left Y",
              "WIDTH": "Width", "HEIGHT": "Height"}
    return {key: int(re.search(rf"{label}:\s+(-?\d+)", info)[1])
            for key, label in fields.items()}


def screenshot(path):
    import gi

    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk

    root = Gdk.get_default_root_window()
    Gdk.pixbuf_get_from_window(root, 0, 0, root.get_width(), root.get_height()).savev(
        str(path), "png", [], []
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--header-x", type=int, default=140)
    parser.add_argument("--header-y", type=int, default=32)
    parser.add_argument("--delta-x", type=int, default=120)
    parser.add_argument("--delta-y", type=int, default=60)
    args = parser.parse_args()
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        parser.error("requires an isolated X11 DISPLAY with WAYLAND_DISPLAY unset")
    args.output.mkdir(parents=True, exist_ok=True)
    windows = xdo("search", "--onlyvisible", "--name", "^Ropy$").splitlines()
    if len(windows) != 1:
        raise RuntimeError(f"expected one visible Ropy window, got {windows}")
    window = windows[0]
    before = geometry(window)
    screenshot(args.output / "before.png")
    start_x, start_y = before["X"] + args.header_x, before["Y"] + args.header_y
    xdo("mousemove", "--sync", start_x, start_y)
    xdo("mousedown", 1)
    try:
        time.sleep(0.3)  # Allow the asynchronous WM move request to acquire its grab.
        # Several real pointer events let the WM acquire its move grab.
        for step in range(1, 13):
            xdo("mousemove", start_x + round(step * args.delta_x / 12),
                start_y + round(step * args.delta_y / 12))
            time.sleep(0.05)
    finally:
        xdo("mouseup", 1)
    time.sleep(0.3)
    after = geometry(window)
    screenshot(args.output / "after.png")
    visible = "IsViewable" in subprocess.check_output(
        ["xwininfo", "-id", window], text=True)
    result = {
        "display": os.environ["DISPLAY"], "window": window,
        "before": before, "after": after, "visible": visible,
        "delta": [after["X"] - before["X"], after["Y"] - before["Y"]],
    }
    (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    assert visible, "dragging hid the window"
    assert abs(result["delta"][0] - args.delta_x) <= 15, "window did not follow horizontal drag"
    assert abs(result["delta"][1] - args.delta_y) <= 15, "window did not follow vertical drag"
    assert (before["WIDTH"], before["HEIGHT"]) == (after["WIDTH"], after["HEIGHT"]), "drag resized the window"


if __name__ == "__main__":
    main()
