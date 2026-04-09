#!/usr/bin/env python3
"""
Assets icon file checker.

Checks two things:
    1. Whether every icon file in assets is actually referenced in the Rust source
         or explicitly allowlisted for external runtime consumers.
  2. Whether every icon reference in source code exists in the assets directory.

Supported icon formats: svg, png, ico, icns, jpg, jpeg, webp, gif, bmp

Usage:
    python3 scripts/check_icons.py [--root <project-root>]

Exit code is non-zero when any issue is found.
"""

import argparse
import re
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

ICON_EXTENSIONS = {".svg", ".png", ".ico", ".icns", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"}
ALLOWLIST_PATH = Path("scripts/check/icon_allowlist.txt")


def collect_asset_icons(assets_dir: Path) -> set[str]:
    """Return a set of all icon file paths (relative to assets dir) found in assets."""
    icons: set[str] = set()
    if not assets_dir.exists():
        return icons

    for ext in ICON_EXTENSIONS:
        for icon_path in assets_dir.rglob(f"*{ext}"):
            # Skip locale files directory
            if "locales" in icon_path.parts:
                continue
            # Get relative path from assets dir
            rel_path = icon_path.relative_to(assets_dir)
            # Use forward slashes for consistency
            icons.add(str(rel_path).replace("\\", "/"))
    return icons


def load_allowlisted_icons(config_path: Path) -> set[str]:
    """Return icon paths explicitly allowlisted for external runtime consumers."""
    icons: set[str] = set()
    if not config_path.exists():
        return icons

    with config_path.open(encoding="utf-8") as f:
        for line in f:
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            icons.add(line.replace("\\", "/"))

    return icons


def collect_referenced_icons(src_root: Path) -> set[str]:
    """Return the set of icon paths referenced anywhere under *src_root*.

    Recognises these patterns:
      * Assets::get("path")
      * Assets::get("path").ok_or(...)
      * .get("path")  (where context suggests it's asset-related)
      * include_str!("path") or include_bytes!("path")
      * .icon(Icon::empty().path("path")) or .path("path")
    """
    # Pattern for Assets::get("path")
    assets_get_pattern = re.compile(r'Assets::get\(\s*"([^"]+)"\s*\)')
    # Pattern for include_str! and include_bytes!
    include_pattern = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)')
    # Pattern for .path("...") method calls (used in icon loading)
    path_pattern = re.compile(r'\.path\(\s*"([^"]+)"\s*\)')

    referenced: set[str] = set()

    for rs_file in src_root.rglob("*.rs"):
        text = rs_file.read_text(encoding="utf-8")

        # Match Assets::get("path")
        for m in assets_get_pattern.finditer(text):
            referenced.add(m.group(1))

        # Match include_str! and include_bytes!
        for m in include_pattern.finditer(text):
            path = m.group(1)
            # Only consider paths that look like icon files
            if any(path.lower().endswith(ext) for ext in ICON_EXTENSIONS):
                referenced.add(path)

        # Match .path("...") calls
        for m in path_pattern.finditer(text):
            path = m.group(1)
            # Only consider paths that look like icon files
            if any(path.lower().endswith(ext) for ext in ICON_EXTENSIONS):
                referenced.add(path)

    return referenced


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------


def check_unused_icons(asset_icons: set[str], referenced_icons: set[str]) -> list[str]:
    """Return icons that exist in assets but are never referenced in source."""
    return sorted([icon for icon in asset_icons if icon not in referenced_icons])


def check_missing_icons(asset_icons: set[str], referenced_icons: set[str]) -> list[str]:
    """Return icons that are referenced in source but don't exist in assets."""
    return sorted([icon for icon in referenced_icons if icon not in asset_icons])


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

_RED = "\033[31m"
_YELLOW = "\033[33m"
_GREEN = "\033[32m"
_BOLD = "\033[1m"
_RESET = "\033[0m"


def _color(text: str, code: str) -> str:
    return f"{code}{text}{_RESET}"


def print_section(title: str) -> None:
    print(f"\n{_BOLD}{title}{_RESET}")
    print("─" * len(title))


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default=".",
        help="Project root directory (default: current directory)",
    )
    args = parser.parse_args()

    root = Path(args.root).resolve()
    assets_dir = root / "assets"
    src_dir = root / "src"

    # Sanity checks
    if not assets_dir.is_dir():
        print(_color(f"ERROR: assets directory not found: {assets_dir}", _RED))
        return 1
    if not src_dir.is_dir():
        print(_color(f"ERROR: source directory not found: {src_dir}", _RED))
        return 1

    asset_icons = collect_asset_icons(assets_dir)
    allowlisted_icons = load_allowlisted_icons(root / ALLOWLIST_PATH)
    referenced_icons = collect_referenced_icons(src_dir) | allowlisted_icons

    issues = 0  # count of problem categories
    verbose = False  # only print details when issues found

    # ------------------------------------------------------------------
    # Check 1 – unused icons in assets
    # ------------------------------------------------------------------
    unused = check_unused_icons(asset_icons, referenced_icons)
    if unused:
        issues += 1
        verbose = True

    # ------------------------------------------------------------------
    # Check 2 – missing icons (referenced but not in assets)
    # ------------------------------------------------------------------
    missing = check_missing_icons(asset_icons, referenced_icons)
    if missing:
        issues += 1
        verbose = True

    # ------------------------------------------------------------------
    # Output (verbose only when issues found)
    # ------------------------------------------------------------------
    if verbose:
        if unused:
            print_section("Check 1 · Icons in assets not referenced in source code")
            for icon in unused:
                print(f"  {_color('UNUSED', _YELLOW)}  {icon}")

        if missing:
            print_section("Check 2 · Icons referenced in source but missing from assets")
            for icon in missing:
                print(f"  {_color('MISSING', _RED)}  {icon}")

        print()
        print(f"  Total icons in assets: {len(asset_icons)}")
        print(f"  Total icons referenced: {len(referenced_icons)}")
        if allowlisted_icons:
            print(f"  Explicit external icon allowances: {len(allowlisted_icons)}")
        print()
        print(_color(f"✗  {issues} issue(s) found.", _RED + _BOLD))
        return 1
    else:
        # Simple summary when everything is OK
        summary = f"✓ icons: {len(asset_icons)} in assets, {len(referenced_icons)} referenced"
        if allowlisted_icons:
            summary += f", {len(allowlisted_icons)} allowlisted external"
        summary += ", all OK"
        print(_color(summary, _GREEN + _BOLD))
        return 0


if __name__ == "__main__":
    sys.exit(main())
