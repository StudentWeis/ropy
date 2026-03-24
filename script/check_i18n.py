#!/usr/bin/env python3
"""
i18n locale file checker.

Checks two things:
  1. Whether every key in en.toml is actually referenced in the Rust source.
  2. Whether every non-template locale file has exactly the same keys as en.toml
     (no missing keys, no extra keys).

Usage:
    python3 script/check_i18n.py [--root <project-root>]

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
    the project's locale files are flat key-value files.
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


def collect_used_keys(src_root: Path) -> set[str]:
    """Return the set of i18n keys referenced anywhere under *src_root*.

    Recognises these call patterns:
      * I18n::translate(cx, "key")
      * i18n.t("key")
      * translations.get("key")
      * any_field_key: "key"  (struct field ending with ``_key``, used for
                               indirect i18n lookups like ``i18n.t(row.label_key)``)
    """
    # Direct I18n::translate / .t() / .get() call patterns
    direct_pattern = re.compile(r'I18n::translate\(\s*\w+\s*,\s*"([^"]+)"\s*\)|\.t\(\s*"([^"]+)"\s*\)|\.get\(\s*"([^"]+)"\s*\)')
    # Struct field whose name ends with ``_key`` assigned a string literal,
    # e.g. ``label_key: "help_search"``
    field_key_pattern = re.compile(r'\b\w+_key\s*:\s*"([^"]+)"')
    used: set[str] = set()
    for rs_file in src_root.rglob("*.rs"):
        text = rs_file.read_text(encoding="utf-8")
        for m in direct_pattern.finditer(text):
            key = m.group(1) or m.group(2) or m.group(3)
            used.add(key)
        for m in field_key_pattern.finditer(text):
            used.add(m.group(1))
    return used


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def check_unused_keys(template_keys: list[str], used_keys: set[str]) -> list[str]:
    """Return keys that exist in the template but are never used in source."""
    return [k for k in template_keys if k not in used_keys]


def check_locale_consistency(
    template_keys: list[str],
    locale_path: Path,
) -> tuple[list[str], list[str]]:
    """Return (missing_keys, extra_keys) for a non-template locale file."""
    template_set = set(template_keys)
    locale_keys = load_toml_keys(locale_path)
    locale_set = set(locale_keys)

    missing = [k for k in template_keys if k not in locale_set]
    extra = [k for k in locale_keys if k not in template_set]
    return missing, extra


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
    locales_dir = root / "assets" / "locales"
    src_dir = root / "src"
    template_path = locales_dir / "en.toml"

    # Sanity checks
    if not template_path.exists():
        print(_color(f"ERROR: template not found: {template_path}", _RED))
        return 1
    if not src_dir.is_dir():
        print(_color(f"ERROR: source directory not found: {src_dir}", _RED))
        return 1

    template_keys = load_toml_keys(template_path)
    used_keys = collect_used_keys(src_dir)

    issues = 0  # count of problem categories
    verbose = False  # only print details when issues found

    # Collect all check results first
    check_results = []

    # ------------------------------------------------------------------
    # Check 1 – unused keys in template
    # ------------------------------------------------------------------
    unused = check_unused_keys(template_keys, used_keys)
    if unused:
        issues += 1
        verbose = True
        check_results.append(("Check 1 · Keys in en.toml not referenced in source code", "unused", unused))

    # ------------------------------------------------------------------
    # Check 2 – key parity across locales
    # ------------------------------------------------------------------
    other_locales = sorted(
        p for p in locales_dir.glob("*.toml") if p.name != "en.toml"
    )

    locale_issues_list = []
    if other_locales:
        for locale_path in other_locales:
            missing, extra = check_locale_consistency(template_keys, locale_path)
            if missing or extra:
                issues += 1
                verbose = True
                rel = locale_path.relative_to(root)
                locale_issues_list.append((rel, missing, extra))

    # ------------------------------------------------------------------
    # Output (verbose only when issues found)
    # ------------------------------------------------------------------
    if verbose:
        # Print Check 1 details
        if unused:
            print_section("Check 1 · Keys in en.toml not referenced in source code")
            for k in unused:
                print(f"  {_color('UNUSED', _YELLOW)}  {k}")

        # Print Check 2 details
        if locale_issues_list:
            print_section("Check 2 · Key parity between en.toml and other locales")
            for rel, missing, extra in locale_issues_list:
                print(f"\n  {_BOLD}{rel}{_RESET}")
                for k in missing:
                    print(f"    {_color('MISSING', _RED)}  {k}")
                for k in extra:
                    print(f"    {_color('EXTRA  ', _YELLOW)}  {k}")

        print()
        print(_color(f"✗  {issues} issue(s) found.", _RED + _BOLD))
        return 1
    else:
        # Simple summary when everything is OK
        num_locales = len(other_locales)
        print(_color(f"✓ i18n: {len(template_keys)} keys, {num_locales} locale(s), all OK", _GREEN + _BOLD))
        return 0


if __name__ == "__main__":
    sys.exit(main())
