#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
	echo "Usage: $0 <apple-darwin-target>" >&2
	exit 2
fi

bundle_target="$1"
case "$bundle_target" in
aarch64-apple-darwin | x86_64-apple-darwin) ;;
*)
	echo "Unsupported macOS target: $bundle_target" >&2
	exit 2
	;;
esac

cargo bundle --release --target "$bundle_target" --format osx

# Restrict the source to this target; a shared workspace can contain both architectures.
app="target/$bundle_target/release/bundle/osx/Ropy.app"
if [[ ! -d "$app" ]]; then
	echo "cargo-bundle did not produce $app" >&2
	exit 1
fi

mkdir -p target/distrib
stage_dir="$(mktemp -d)"
trap 'rm -rf "$stage_dir"' EXIT
cp -R "$app" "$stage_dir/"
ln -s /Applications "$stage_dir/Applications"
hdiutil create -volname "Ropy" -srcfolder "$stage_dir" -ov -format UDZO \
	"target/distrib/ropy-$bundle_target.dmg"

# Preserve the complete application for in-app updates, independently of the
# binary-only cargo-dist archives used by standalone installations.
archive="target/distrib/ropy-$bundle_target-app.tar.xz"
COPYFILE_DISABLE=1 tar -cJf "$archive" -C "$(dirname "$app")" Ropy.app
(cd target/distrib && shasum -a 256 "${archive##*/}" >"${archive##*/}.sha256")
