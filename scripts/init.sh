#!/usr/bin/env bash
set -e

# Base dependencies
cargo install cargo-binstall

# Other dependencies
cargo binstall prek cargo-dist git-cliff

# For pre-commit hooks
prek install
