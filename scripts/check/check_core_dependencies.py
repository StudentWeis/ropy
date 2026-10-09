#!/usr/bin/env python3
"""Reject desktop dependencies in the core package, including target-specific edges."""

from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
DESKTOP_PACKAGES = {
    "clipboard-rs", "global-hotkey", "tray-icon", "enigo", "x11rb",
    "gtk", "gtk-sys", "gtk4", "gtk4-sys", "gdk", "gdk-sys",
}


def main():
    result = subprocess.run(
        ["cargo", "tree", "--locked", "-p", "ropy-core", "--all-features",
         "--target", "all", "--edges", "normal,build,dev", "--prefix", "none",
         "--format", "{p}"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        return result.returncode
    packages = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
    forbidden = sorted(name for name in packages
                       if name in DESKTOP_PACKAGES or name.startswith("gpui"))
    if forbidden:
        print("Core depends on desktop packages: " + ", ".join(forbidden), file=sys.stderr)
        return 1
    print("Core dependency boundary OK (all features and target platforms).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
