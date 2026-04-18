#!/usr/bin/env bash
set -euo pipefail

usage() {
	cat <<'EOF'
Usage: scripts/update_version.sh <level|version> [cargo-release args...]

Examples:
  scripts/update_version.sh patch                        # dry-run patch bump
  scripts/update_version.sh 0.6.0 --execute              # release 0.6.0
  scripts/update_version.sh 0.6.0-beta --execute          # pre-release
  scripts/update_version.sh rc --execute --no-confirm     # release candidate

This wrapper delegates to cargo release.
Dry-run mode is the default and automatically adds --no-verify to avoid
leaving Cargo.lock behind in repositories that do not track lockfiles.
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ "$#" -eq 0 ]; then
	usage >&2
	exit 1
fi

case "$1" in
-h | --help)
	usage
	exit 0
	;;
esac

cd "$REPO_ROOT"

release_args=("$@")
execute_requested=false
no_verify_requested=false

for arg in "${release_args[@]}"; do
	case "$arg" in
	-x | --execute)
		execute_requested=true
		;;
	--no-verify)
		no_verify_requested=true
		;;
	esac
done

if [ "$execute_requested" = false ] && [ "$no_verify_requested" = false ]; then
	release_args+=("--no-verify")
fi

if [ "$execute_requested" = false ]; then
	echo "Running cargo release dry-run (release preparation hook will skip mutations)."
fi

echo "Running: cargo release ${release_args[*]}"
exec cargo release "${release_args[@]}"
