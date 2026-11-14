#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
DIST_VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("dist-workspace.toml", "rb"))["dist"]["cargo-dist-version"])')"
# hk.pkl pins the Pkl config package to a release; install the same hk version so
# the builtin definitions and the binary cannot drift apart.
HK_VERSION="$(sed -n 's/^min_hk_version = "\(.*\)"$/\1/p' hk.pkl)"
if [[ -z "$HK_VERSION" ]]; then
	echo "hk.pkl must declare min_hk_version; setup installs the matching hk version from it." >&2
	exit 1
fi

# Parser hooks use uv's isolated Python tool environments.
if ! command -v uvx &>/dev/null; then
	echo "Install uv before running setup: https://docs.astral.sh/uv/getting-started/installation/" >&2
	exit 1
fi

# Base dependencies
cargo install cargo-binstall

# Other dependencies
cargo binstall "hk@$HK_VERSION" "cargo-dist@$DIST_VERSION" cargo-release git-cliff cargo-outdated

# For pre-commit hooks
# Replace the previous hook script, including on Git with config-based hooks.
hk install --legacy --force
