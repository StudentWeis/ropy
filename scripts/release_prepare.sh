#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

read_package_version() {
	perl -0777 -ne 'print "$1\n" and exit if /\[package\]\n(?:.*\n)*?version\s*=\s*"([^"]+)"/m' Cargo.toml
}

if [[ "${DRY_RUN:-false}" == "true" ]]; then
	echo "Skipping release preparation during cargo release dry-run."
	exit 0
fi

release_version="${NEW_VERSION:-$(read_package_version)}"

if [[ -z "$release_version" ]]; then
	echo "Error: failed to resolve release version from Cargo.toml" >&2
	exit 1
fi

# Sync version to [package.metadata.bundle.bin.ropy]
perl -i -0777 -pe "s/(\[package\.metadata\.bundle\.bin\.ropy\]\n(?:.*\n)*?version\s*=\s*\").*?\"/\${1}$release_version\"/m" Cargo.toml

# Check code formatting, linting, and tests before release
./scripts/precheck.sh
./scripts/record_build_size.sh

# Generate changelog for the new release
git cliff --unreleased --tag "$release_version" --prepend CHANGELOG.md
# Remove HTML comment markers (e.g., <!-- 0 -->) from CHANGELOG.md
# These markers are added by git-cliff as placeholders for version numbers
perl -i -pe 's/<!-- \d+ -->//g' CHANGELOG.md

# Verify dist plan
dist plan

# Ask for confirmation before proceeding with the release
read -p "Continue with release? [Y/n] " -n 1 -r confirm
confirm="${confirm:-y}"
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
	echo -e "\nRelease cancelled."
	exit 0
fi
echo -e "\nRelease confirmed, continuing..."
