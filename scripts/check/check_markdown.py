#!/usr/bin/env python3
"""Verify relative links and heading anchors in repository Markdown.

Checks every tracked Markdown file except the vendored GPUI Kit skill
directories, which are upstream copies and keep upstream site-relative links.
Absolute URLs, `mailto:` targets and paths starting with `/` are ignored.
Fenced code blocks and inline code spans are ignored so examples cannot fail
the check. Anchors are validated only for Markdown targets; duplicate headings
are not disambiguated with GitHub's numeric suffixes.

Usage:
    python3 scripts/check/check_markdown.py [--root <project-root>]

Exit code is non-zero when any link or anchor cannot be resolved.
"""

import argparse
import re
import sys
from pathlib import Path
from urllib.parse import unquote


# Upstream copies keep upstream links; ropy-gpui records that boundary.
SKIPPED_PREFIXES = (
    ".agents/skills/gpui-kit/",
    ".agents/skills/gpui-kit-design-guides/",
)
SKIPPED_DIRECTORIES = {".git", "target", "node_modules"}
IGNORED_TARGET_PREFIXES = ("http://", "https://", "mailto:", "/", "//")

LINK = re.compile(r"\[[^\]]*\]\(\s*<?([^()\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
INLINE_CODE = re.compile(r"`[^`]*`")
HEADING = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$")
FENCE = re.compile(r"^\s*(```|~~~)")


def markdown_files(root: Path) -> list[Path]:
    """Return Markdown files in a stable order, skipping vendored snapshots."""
    files: list[Path] = []
    for path in sorted(root.rglob("*.md")):
        relative = path.relative_to(root).as_posix()
        if SKIPPED_DIRECTORIES & set(path.parts):
            continue
        if any(relative.startswith(prefix) for prefix in SKIPPED_PREFIXES):
            continue
        files.append(path)
    return files


def strip_inline_code(line: str) -> str:
    """Drop inline code spans, whose contents are examples rather than links."""
    return INLINE_CODE.sub("", line)


def meaningful_lines(text: str) -> list[tuple[int, str]]:
    """Return (line number, line) pairs outside fenced code blocks."""
    lines: list[tuple[int, str]] = []
    in_fence = False
    for number, line in enumerate(text.splitlines(), start=1):
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if not in_fence:
            lines.append((number, line))
    return lines


def heading_slug(heading: str) -> str:
    """Convert Heading text to the GitHub anchor fragment."""
    slug = re.sub(r"<[^>]+>", "", heading.strip().lower())
    slug = re.sub(r"[^\w\- ]", "", slug)
    return slug.replace(" ", "-")


def anchors(path: Path) -> set[str]:
    """Return the anchor fragments GitHub generates for a file's headings."""
    return {
        heading_slug(match.group(2))
        for _, line in meaningful_lines(path.read_text(encoding="utf-8"))
        if (match := HEADING.match(line))
    }


def check_file(root: Path, path: Path) -> list[str]:
    """Return human-readable problems for one Markdown file."""
    problems: list[str] = []
    cache: dict[Path, set[str]] = {}
    for number, line in meaningful_lines(path.read_text(encoding="utf-8")):
        for target in LINK.findall(strip_inline_code(line)):
            if target.startswith(IGNORED_TARGET_PREFIXES):
                continue
            link, _, fragment = unquote(target).partition("#")
            linked = (path.parent / link).resolve() if link else path.resolve()
            if not linked.is_relative_to(root):
                problems.append(
                    f"{path.relative_to(root)}:{number}: {target} escapes the repository"
                )
                continue
            if not linked.exists():
                problems.append(f"{path.relative_to(root)}:{number}: {target} does not exist")
                continue
            if not fragment or linked.is_dir() or linked.suffix != ".md":
                continue
            if linked not in cache:
                cache[linked] = anchors(linked)
            if fragment not in cache[linked]:
                problems.append(
                    f"{path.relative_to(root)}:{number}: {target} has no matching heading"
                )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default=".",
        help="Project root directory (default: current directory)",
    )
    root = Path(parser.parse_args().root).resolve()

    problems = [
        problem for path in markdown_files(root) for problem in check_file(root, path)
    ]
    if problems:
        for problem in problems:
            print(problem)
        print(f"\n{len(problems)} broken Markdown link(s) or anchor(s).")
        return 1
    print(f"markdown: {len(markdown_files(root))} files, links and anchors resolve")
    return 0


if __name__ == "__main__":
    sys.exit(main())
