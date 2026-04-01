#!/usr/bin/env python3
"""
Theme file checker.

Checks three things:
  1. Whether every theme file has all required color keys defined.
  2. Whether all theme files have consistent keys with the template theme (Ropy Light).
  3. Whether all color values are valid hex colors.

Usage:
    python3 scripts/check_themes.py [--root <project-root>]

Exit code is non-zero when any issue is found.
"""
import argparse
import re
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def load_toml_keys(path: Path) -> list[str]:
    """Return an ordered list of top-level string keys from a TOML file.

    This is intentionally kept dependency-free: it only parses simple
    ``key = "value"`` lines.  Section headers ([table]) are ignored because
    the project's theme files are flat key-value files.
    """
    keys: list[str] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            # Skip comments and blank lines
            if not line or line.startswith("#"):
                continue
            # Skip TOML section headers
            if line.startswith("["):
                continue
            m = re.match(r'^([A-Za-z0-9_\-]+)\s*=', line)
            if m:
                keys.append(m.group(1))
    return keys

def load_toml_key_values(path: Path) -> dict[str, str]:
    """Return a dict of key-value pairs from a TOML file.

    Only parses simple ``key = "value"`` or ``key = value`` lines.
    """
    kv: dict[str, str] = {}
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            # Skip comments and blank lines
            if not line or line.startswith("#"):
                continue
            # Skip TOML section headers
            if line.startswith("["):
                continue
            m = re.match(r'^([A-Za-z0-9_\-]+)\s*=\s*(.+)$', line)
            if m:
                key = m.group(1)
                value = m.group(2).strip()
                kv[key] = value
    return kv

# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def check_theme_consistency(
    template_keys: list[str],
    theme_path: Path,
) -> tuple[list[str], list[str]]:
    """Return (missing_keys, extra_keys) for a theme file."""
    template_set = set(template_keys)
    theme_keys = load_toml_keys(theme_path)
    theme_set = set(theme_keys)

    missing = [k for k in template_keys if k not in theme_set]
    extra = [k for k in theme_keys if k not in template_set]
    return missing, extra

def is_valid_hex_color(value: str) -> bool:
    """Check if a value is a valid hex color.

    Valid formats:
    - "#RGB" (3-digit hex)
    - "#RRGGBB" (6-digit hex)
    - "#RRGGBBAA" (8-digit hex with alpha)
    """
    # Remove quotes if present
    if (value.startswith('"') and value.endswith('"')) or \
       (value.startswith("'") and value.endswith("'")):
        value = value[1:-1]

    # Check if it starts with #
    if not value.startswith('#'):
        return False

    # Remove the # prefix
    hex_value = value[1:]

    # Check length and if all characters are valid hex digits
    if len(hex_value) not in [3, 6, 8]:
        return False

    # Check if all characters are valid hex digits
    try:
        int(hex_value, 16)
        return True
    except ValueError:
        return False

def check_invalid_colors(theme_path: Path) -> list[tuple[str, str]]:
    """Return list of (key, value) pairs that have invalid hex color values.

    Only checks color keys (excludes theme_name and mode).
    """
    kv = load_toml_key_values(theme_path)
    invalid_colors = []

    for key, value in kv.items():
        # Skip meta keys
        if key in ["theme_name", "mode"]:
            continue

        # Check if value is a valid hex color
        if not is_valid_hex_color(value):
            invalid_colors.append((key, value))

    return invalid_colors

def check_empty_values(theme_path: Path) -> list[str]:
    """Return keys that have empty or placeholder values."""
    kv = load_toml_key_values(theme_path)
    empty_keys = []

    for key, value in kv.items():
        # Skip meta keys
        if key in ["theme_name", "mode"]:
            continue

        # Check for empty string
        if value == '""' or value == "''":
            empty_keys.append(key)
            continue

        # Check for placeholder values (e.g., TODO, TBD, etc.)
        if value.upper() in ['"TODO"', '"TBD"', '"FIXME"', '"PLACEHOLDER"', "'TODO'", "'TBD'", "'FIXME'", "'PLACEHOLDER'"]:
            empty_keys.append(key)
            continue

        # Check for unquoted placeholder values
        if value.upper() in ['TODO', 'TBD', 'FIXME', 'PLACEHOLDER']:
            empty_keys.append(key)
            continue

    return empty_keys

# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

_RED   = "\033[31m"
_YELLOW = "\033[33m"
_GREEN = "\033[32m"
_BOLD  = "\033[1m"
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
    themes_dir = root / "assets" / "themes"
    template_path = themes_dir / "ropy-light.toml"

    # Sanity checks
    if not template_path.exists():
        print(_color(f"ERROR: template theme not found: {template_path}", _RED))
        return 1
    if not themes_dir.is_dir():
        print(_color(f"ERROR: themes directory not found: {themes_dir}", _RED))
        return 1

    template_keys = load_toml_keys(template_path)

    issues = 0  # count of problem categories
    verbose = False  # only print details when issues found

    # ------------------------------------------------------------------
    # Check 1 – key parity across themes
    # ------------------------------------------------------------------
    other_themes = sorted(
        p for p in themes_dir.glob("*.toml") if p.name != "ropy-light.toml"
    )

    theme_issues_list = []
    if other_themes:
        for theme_path in other_themes:
            missing, extra = check_theme_consistency(template_keys, theme_path)
            if missing or extra:
                issues += 1
                verbose = True
                rel = theme_path.relative_to(root)
                theme_issues_list.append((rel, missing, extra))

    # ------------------------------------------------------------------
    # Check 2 – empty or placeholder values
    # ------------------------------------------------------------------
    empty_values_list = []
    all_themes = [template_path] + other_themes
    for theme_path in all_themes:
        empty_keys = check_empty_values(theme_path)
        if empty_keys:
            issues += 1
            verbose = True
            rel = theme_path.relative_to(root)
            empty_values_list.append((rel, empty_keys))

    # ------------------------------------------------------------------
    # Check 3 – invalid hex color values
    # ------------------------------------------------------------------
    invalid_colors_list = []
    for theme_path in all_themes:
        invalid_colors = check_invalid_colors(theme_path)
        if invalid_colors:
            issues += 1
            verbose = True
            rel = theme_path.relative_to(root)
            invalid_colors_list.append((rel, invalid_colors))

    # ------------------------------------------------------------------
    # Output (verbose only when issues found)
    # ------------------------------------------------------------------
    if verbose:
        # Print Check 1 details
        if theme_issues_list:
            print_section("Check 1 · Key parity between ropy-light.toml and other themes")
            for rel, missing, extra in theme_issues_list:
                print(f"\n  {_BOLD}{rel}{_RESET}")
                for k in missing:
                    print(f"    {_color('MISSING', _RED)}  {k}")
                for k in extra:
                    print(f"    {_color('EXTRA  ', _YELLOW)}  {k}")

        # Print Check 2 details
        if empty_values_list:
            print_section("Check 2 · Empty or placeholder values in theme files")
            for rel, empty_keys in empty_values_list:
                print(f"\n  {_BOLD}{rel}{_RESET}")
                for k in empty_keys:
                    print(f"    {_color('EMPTY  ', _YELLOW)}  {k}")

        # Print Check 3 details
        if invalid_colors_list:
            print_section("Check 3 · Invalid hex color values in theme files")
            for rel, invalid_colors in invalid_colors_list:
                print(f"\n  {_BOLD}{rel}{_RESET}")
                for k, v in invalid_colors:
                    print(f"    {_color('INVALID', _RED)}  {k} = {v}")

        print()
        print(_color(f"✗  {issues} issue(s) found.", _RED + _BOLD))
        return 1
    else:
        # Simple summary when everything is OK
        num_themes = len(all_themes)
        print(_color(f"✓ themes: {len(template_keys)} keys, {num_themes} theme(s), all OK", _GREEN + _BOLD))
        return 0

if __name__ == "__main__":
    sys.exit(main())
