#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   scripts/precheck.sh                  # local: fix formatting, then all checks
#   scripts/precheck.sh --check          # CI: verify without changing files
#   scripts/precheck.sh --check --light  # formatting, resources, scripts
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

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
RUSTFMT_TOOLCHAIN="$(cat rustfmt-toolchain)"

# Determine which command to use for Rust operations
if command -v rtk &>/dev/null; then
	CARGO_CMD="rtk cargo"
else
	CARGO_CMD="cargo"
fi

if [[ "$PHASE" != rust ]]; then
	if [[ $CHECK_ONLY -eq 1 ]]; then
		$CARGO_CMD "+$RUSTFMT_TOOLCHAIN" fmt --all --check
	else
		$CARGO_CMD "+$RUSTFMT_TOOLCHAIN" fmt --all
		if command -v shfmt &>/dev/null; then
			shfmt -w ./**/*.sh
		fi
	fi

	# Fail on inexpensive checks before compiling any application code.
	python3 scripts/check/check_i18n.py
	python3 scripts/check/check_icons.py
	python3 scripts/check/check_themes.py
	python3 -m unittest discover -s scripts/tests
fi

if [[ "$PHASE" != light ]]; then
	python3 scripts/check/check_core_dependencies.py
	# Clippy already performs the compiler checks for the same target/feature set.
	$CARGO_CMD clippy --locked --workspace --all-targets --all-features
	$CARGO_CMD test --locked --workspace --all-targets --all-features
	if [[ "$(uname -s)" == Linux ]]; then
		if ! command -v dbus-run-session &>/dev/null; then
			echo "Linux precheck requires dbus-run-session; install your distribution's D-Bus tools." >&2
			exit 1
		fi
		echo "Testing Linux tray on a private session bus"
		dbus-run-session -- $CARGO_CMD test --locked -p ropy test_tray_without_gtk_initializes_refreshes_and_drops -- --ignored
	fi
	RUSTDOCFLAGS="-D warnings" $CARGO_CMD doc --locked --workspace --no-deps
fi
