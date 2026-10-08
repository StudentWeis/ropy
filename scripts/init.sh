#!/usr/bin/env bash
set -e

# Parser hooks use uv's isolated Python tool environments.
if ! command -v uvx &>/dev/null; then
	echo "Install uv before running setup: https://docs.astral.sh/uv/getting-started/installation/" >&2
	exit 1
fi

# Base dependencies
cargo install cargo-binstall

# Other dependencies
cargo binstall 'hk@^2.5' cargo-dist cargo-release git-cliff cargo-machete cargo-outdated

# For pre-commit hooks
# Replace the previous hook script, including on Git with config-based hooks.
hk install --legacy --force
