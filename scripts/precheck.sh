#!/usr/bin/env bash
set -euo pipefail

# Determine which command to use for Rust operations
if command -v rtk &>/dev/null; then
	CARGO_CMD="rtk cargo"
else
	CARGO_CMD="cargo"
fi

$CARGO_CMD +nightly fmt
$CARGO_CMD check
$CARGO_CMD clippy --all-targets --all-features
$CARGO_CMD test -- --test-threads=1
python3 scripts/check/check_i18n.py
python3 scripts/check/check_icons.py
python3 scripts/check/check_themes.py
