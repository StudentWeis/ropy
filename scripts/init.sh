#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
DIST_VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("dist-workspace.toml", "rb"))["dist"]["cargo-dist-version"])')"

# Parser hooks use uv's isolated Python tool environments.
if ! command -v uvx &>/dev/null; then
	echo "Install uv before running setup: https://docs.astral.sh/uv/getting-started/installation/" >&2
	exit 1
fi

# Base dependencies
cargo install cargo-binstall

# Other dependencies
cargo binstall 'hk@^2.5' "cargo-dist@$DIST_VERSION" cargo-release git-cliff cargo-outdated

# For pre-commit hooks
# Replace the previous hook script, including on Git with config-based hooks.
hk install --legacy --force
