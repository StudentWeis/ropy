#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 6 ]]; then
	echo "Usage: $0 <artifact-dir> <repository> <tag> <notes-file> <output> <dist-plan>" >&2
	exit 2
fi

artifact_dir="$1"
repository="$2"
tag="$3"
notes_file="$4"
output="$5"
assets='[]'

append_asset() {
	local path="$1"
	local name="${path##*/}"
	local size
	local url

	size="$(wc -c <"$path" | tr -d ' ')"
	url="https://github.com/${repository}/releases/download/${tag}/${name}"
	assets="$(
		jq -c \
			--arg name "$name" \
			--arg url "$url" \
			--argjson size "$size" \
			'. + [{name: $name, browser_download_url: $url, size: $size}]' \
			<<<"$assets"
	)"
}

# The dist plan owns the platform matrix. Bundling also produces one complete
# application archive for each planned macOS binary archive.
expected="$(jq -er '[
	.releases[] | select(.app_name == "ropy") | .artifacts[]
	| select(test("^ropy-[A-Za-z0-9_-]+\\.(tar\\.xz|zip)$"))
	| ., (select(endswith("-apple-darwin.tar.xz")) | sub("\\.tar\\.xz$"; "-app.tar.xz"))
] | unique | if length == 0 then error("No update archives in dist plan") else .[] end' "$6")"

while IFS= read -r name; do
	archive="$artifact_dir/$name"
	if [[ ! -f "$archive" ]]; then
		echo "Missing update archive: $archive" >&2
		exit 1
	fi
	append_asset "$archive"
	if [[ ! -f "${archive}.sha256" ]]; then
		echo "Missing checksum for update archive: ${archive}.sha256" >&2
		exit 1
	fi
	append_asset "${archive}.sha256"
done <<<"$expected"

jq -n \
	--arg tag "$tag" \
	--rawfile body "$notes_file" \
	--argjson assets "$assets" \
	'{tag_name: $tag, body: $body, assets: $assets}' \
	>"$output"
