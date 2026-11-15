#!/usr/bin/env python3
"""Verify the vendored GPUI Kit skill snapshots and the recorded versions.

The two official skill directories are unmodified upstream copies, so their
contents are pinned by a digest in `.agents/official-snapshots.json`. The
adapter records the upstream revision and the dependency versions it targets,
which must stay consistent with the manifest, `Cargo.toml` and `Cargo.lock`.

Usage:
    python3 scripts/check/check_agent_skills.py [--root <project-root>] [--update]

`--update` rewrites the digests in the manifest. Use it after replacing a
snapshot directory, then update the revision and versions in the adapter.

Exit code is non-zero when a snapshot, revision or version disagrees.
"""

import argparse
import hashlib
import json
import sys
import tomllib
from pathlib import Path


MANIFEST = Path(".agents/official-snapshots.json")
ADAPTER = Path(".agents/skills/ropy-gpui/SKILL.md")
REQUIRED_SNAPSHOTS = ("gpui-kit", "gpui-kit-design-guides")


def directory_digest(directory: Path) -> str:
    """Return a SHA-256 digest over relative paths and file contents."""
    digest = hashlib.sha256()
    for file in sorted(path for path in directory.rglob("*") if path.is_file()):
        digest.update(file.relative_to(directory).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(file.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def load_manifest(root: Path) -> dict:
    return json.loads((root / MANIFEST).read_text(encoding="utf-8"))


def cargo_versions(root: Path) -> dict[str, str]:
    """Return the locked engine version and the declared gpui-kit version."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    locked = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    engines = {
        package["name"]: package["version"]
        for package in locked["package"]
        if package["name"] in {"gpui-kit", "gpui-pre"}
    }
    return {
        "gpui-kit": manifest["dependencies"]["gpui-kit"]["version"],
        "gpui-pre": engines["gpui-pre"],
    }


def check(root: Path, snapshots: dict) -> list[str]:
    problems: list[str] = [
        f"{MANIFEST}: does not record the {name} snapshot"
        for name in REQUIRED_SNAPSHOTS
        if name not in snapshots
    ]
    adapter = (root / ADAPTER).read_text(encoding="utf-8")
    versions = cargo_versions(root)

    for name, entry in snapshots.items():
        directory = root / entry["path"]
        if not directory.is_dir():
            problems.append(f"{entry['path']}: snapshot directory is missing")
        else:
            actual = directory_digest(directory)
            if actual != entry["sha256"]:
                problems.append(
                    f"{entry['path']}: content differs from the recorded {name} snapshot "
                    f"({actual})"
                )
        if entry["commit"] not in adapter:
            problems.append(f"{ADAPTER}: does not record the {name} commit {entry['commit']}")

    for package, version in versions.items():
        if version not in adapter:
            problems.append(f"{ADAPTER}: does not record {package} {version}")

    return problems


def update(root: Path, snapshots: dict) -> None:
    for entry in snapshots.values():
        entry["sha256"] = directory_digest(root / entry["path"])
    (root / MANIFEST).write_text(
        json.dumps(snapshots, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default=".",
        help="Project root directory (default: current directory)",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="rewrite the recorded digests before checking",
    )
    arguments = parser.parse_args()
    root = Path(arguments.root).resolve()

    snapshots = load_manifest(root)
    if arguments.update:
        update(root, snapshots)
        print(f"agent skills: digests recorded in {MANIFEST}")

    problems = check(root, snapshots)
    if problems:
        for problem in problems:
            print(problem)
        return 1

    names = ", ".join(f"{name} ({entry['sha256'][:12]})" for name, entry in snapshots.items())
    print(f"agent skills: {names} match, versions recorded in the adapter")
    return 0


if __name__ == "__main__":
    sys.exit(main())
