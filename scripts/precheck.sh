#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   scripts/precheck.sh                  # local: fix formatting, then all checks
#   scripts/precheck.sh --check          # CI: verify without changing files
#   scripts/precheck.sh --check --light  # formatting, resources, scripts, dependencies
#   scripts/precheck.sh --check --rust   # clippy, tests, documentation

CHECK_ONLY=0
PHASE=all
for arg in "$@"; do
	case "$arg" in
	--check) CHECK_ONLY=1 ;;
	--light | --rust)
		if [[ "$PHASE" != all ]]; then
			echo "Choose only one of --light or --rust" >&2
			exit 2
		fi
		PHASE="${arg#--}"
		;;
	*)
		echo "Unknown precheck option: $arg" >&2
		exit 2
		;;
	esac
done

# Determine which command to use for Rust operations
if command -v rtk &>/dev/null; then
	CARGO_CMD="rtk cargo"
else
	CARGO_CMD="cargo"
fi

if [[ "$PHASE" != rust ]]; then
	if [[ $CHECK_ONLY -eq 1 ]]; then
		$CARGO_CMD +nightly fmt --all --check
	else
		$CARGO_CMD +nightly fmt --all
		if command -v shfmt &>/dev/null; then
			shfmt -w ./**/*.sh
		fi
	fi

	# Fail on inexpensive checks before compiling any application code.
	python3 scripts/check/check_i18n.py
	python3 scripts/check/check_icons.py
	python3 scripts/check/check_themes.py
	python3 -m unittest discover -s scripts/tests
	if command -v cargo-machete &>/dev/null; then
		$CARGO_CMD machete
	fi
fi

if [[ "$PHASE" != light ]]; then
	python3 scripts/check/check_core_dependencies.py
	# Clippy already performs the compiler checks for the same target/feature set.
	$CARGO_CMD clippy --workspace --all-targets --all-features
	$CARGO_CMD test --workspace --all-targets --all-features
	RUSTDOCFLAGS="-D warnings" $CARGO_CMD doc --workspace --no-deps
fi
