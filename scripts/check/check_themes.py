#!/usr/bin/env python3
"""Validate bundled themes with Python 3.11+ tomllib and Ropy's runtime schema.

Usage: python3 scripts/check/check_themes.py [--root <project-root>]
"""

import argparse
from pathlib import Path
import re
import sys
import tomllib


# Keep aligned with ThemeDefinitionRaw and parse_color in src/gui/theme.rs.
# Validate the template too: parity alone can miss a field absent from all themes.
COLOR_KEYS = (
    "background", "foreground", "secondary", "secondary_foreground", "border",
    "accent", "accent_foreground", "muted", "muted_foreground", "input",
    "primary", "primary_foreground", "primary_hover", "primary_active",
    "danger", "danger_foreground", "popover", "popover_foreground",
    "selection", "ring", "list_hover", "list_active", "scrollbar_thumb",
)
REQUIRED_KEYS = {"theme_name", "mode", *COLOR_KEYS}


def validate_theme(theme: dict[str, object]) -> list[str]:
    """Return schema errors after TOML syntax has been checked by tomllib."""
    errors = [f"MISSING {key}" for key in sorted(REQUIRED_KEYS - theme.keys())]
    # Runtime serde ignores unknown keys; the repository gate intentionally
    # retains its stricter typo/extra-key check.
    errors.extend(f"EXTRA {key}" for key in sorted(theme.keys() - REQUIRED_KEYS))
    if "theme_name" in theme and not isinstance(theme["theme_name"], str):
        errors.append("INVALID theme_name: expected a string")
    if "mode" in theme and theme["mode"] not in ("light", "dark"):
        errors.append("INVALID mode: expected 'light' or 'dark'")
    for key in COLOR_KEYS:
        if key not in theme:
            continue
        value = theme[key]
        # The runtime trims whitespace and accepts six ASCII hex digits with
        # an optional '#'. It accepts neither short RGB nor an alpha channel.
        if not isinstance(value, str) or re.fullmatch(r"#?[0-9a-fA-F]{6}", value.strip()) is None:
            errors.append(f"INVALID {key} = {value!r}: expected six RGB hex digits")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="Project root directory")
    root = Path(parser.parse_args().root).resolve()
    themes_dir = root / "assets" / "themes"
    template_path = themes_dir / "ropy-light.toml"
    if not template_path.is_file():
        print(f"ERROR: template theme not found: {template_path}")
        return 1

    theme_paths = sorted(themes_dir.glob("*.toml"))
    issues = 0
    for path in theme_paths:
        try:
            with path.open("rb") as source:
                theme = tomllib.load(source)
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            errors = [f"TOML/read error: {error}"]
        else:
            errors = validate_theme(theme)
        for error in errors:
            print(f"{path.relative_to(root)}: {error}")
        issues += len(errors)

    if issues:
        print(f"✗ themes: {issues} issue(s) found")
        return 1
    print(f"✓ themes: {len(REQUIRED_KEYS)} keys, {len(theme_paths)} theme(s), all OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
