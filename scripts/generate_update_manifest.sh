#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
	echo "Usage: $0 <artifact-dir> <repository> <tag> <notes-file> <output>" >&2
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

while IFS= read -r -d '' archive; do
	append_asset "$archive"
	if [[ -f "${archive}.sha256" ]]; then
		append_asset "${archive}.sha256"
	fi
done < <(
	find "$artifact_dir" -maxdepth 1 -type f \
		\( -name 'ropy-*.tar.xz' -o -name 'ropy-*.zip' \) \
		-print0 | sort -z
)

if [[ "$assets" == '[]' ]]; then
	echo "No update archives found in $artifact_dir" >&2
	exit 1
fi

jq -n \
	--arg tag "$tag" \
	--rawfile body "$notes_file" \
	--argjson assets "$assets" \
	'{tag_name: $tag, body: $body, assets: $assets}' \
	>"$output"
