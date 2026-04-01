#!/usr/bin/env bash
set -euo pipefail

# Check if a version argument is provided
if [ -z "$1" ]; then
	echo "Usage: $0 <new_version>"
	echo "Example: $0 0.1.2"
	exit 1
fi

NEW_VERSION=$1
CARGO_TOML="Cargo.toml"

# Check if Cargo.toml exists
if [ ! -f "$CARGO_TOML" ]; then
	echo "Error: $CARGO_TOML file not found"
	exit 1
fi

echo "Updating version to: $NEW_VERSION ..."

# Use perl for replacement because it is more reliable in handling multiline patterns and cross-platform (macOS/Linux) than sed
# 1. Replace version under [package]
perl -i -0777 -pe "s/(\[package\]\n(?:.*\n)*?version\s*=\s*\").*?\"/\${1}$NEW_VERSION\"/m" "$CARGO_TOML"

# 2. Replace version under [package.metadata.bundle]
perl -i -0777 -pe "s/(\[package\.metadata\.bundle\]\n(?:.*\n)*?version\s*=\s*\").*?\"/\${1}$NEW_VERSION\"/m" "$CARGO_TOML"

# 3. Update CHANGELOG.md using git cliff
git cliff --unreleased --tag $NEW_VERSION --prepend CHANGELOG.md

# 4. Confirm before push git tag
dist plan
while true; do
	read -r -p "Continue? [Y/n] " REPLY
	REPLY=${REPLY:-Y}
	case "$REPLY" in
	[Yy]*)
		git add -A
		git commit -m "chore: update version to $NEW_VERSION"
		git tag "$NEW_VERSION"
		git push --tags
		break
		;;
	[Nn]*)
		echo "Release aborted by user."
		exit 0
		;;
	*) echo "Please answer Y or n." ;;
	esac
done

echo "Update completed!"
