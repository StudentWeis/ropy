"""Generate a stable-release Cask from downloaded, architecture-specific DMGs."""
import argparse
import hashlib
import json
from pathlib import Path
import re


def render(version: str, assets: Path) -> str:
    """Reject non-stable versions and hash both complete DMG payloads."""
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable release tag such as 0.5.7")
    hashes = {
        arch: hashlib.sha256((assets / f"ropy-{arch}-apple-darwin.dmg").read_bytes()).hexdigest()
        for arch in ("aarch64", "x86_64")
    }
    return render_hashes(version, hashes)


def render_release(release: dict) -> str:
    """Use GitHub-published SHA-256 digests; never accept absent checksums."""
    if release["isDraft"] or release["isPrerelease"]:
        raise ValueError("Expected a published stable release")
    hashes = {}
    for arch in ("aarch64", "x86_64"):
        name = f"ropy-{arch}-apple-darwin.dmg"
        matches = [asset for asset in release["assets"] if asset["name"] == name]
        if len(matches) != 1:
            raise ValueError(f"Expected one release asset: {name}")
        digest = matches[0].get("digest") or ""
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
            raise ValueError(f"Missing or invalid SHA-256 digest: {name}")
        hashes[arch] = digest.removeprefix("sha256:")
    return render_hashes(release["tagName"], hashes)


def render_hashes(version: str, hashes: dict[str, str]) -> str:
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable release tag such as 0.5.7")
    return f'''cask "ropy" do
  arch arm: "aarch64", intel: "x86_64"

  version "{version}"
  sha256 arm:   "{hashes['aarch64']}",
         intel: "{hashes['x86_64']}"

  url "https://github.com/StudentWeis/ropy/releases/download/#{{version}}/ropy-#{{arch}}-apple-darwin.dmg"
  name "Ropy"
  desc "Clipboard manager built with Rust and GPUI"
  homepage "https://github.com/StudentWeis/ropy"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on :macos

  app "Ropy.app"

  # Cask moves the bundle to /Applications, outside the Homebrew prefix.
  # Use Ropy's existing publisher marker to leave upgrades to Homebrew.
  postflight_steps do
    touch "Ropy.app/Contents/.ropy-managed", base: :appdir
  end

  uninstall quit: "com.ropy.app"
end
'''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-json", type=Path)
    parser.add_argument("version", nargs="?")
    parser.add_argument("assets", nargs="?", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.release_json:
        content = render_release(json.loads(args.release_json.read_text()))
    elif args.version and args.assets:
        content = render(args.version, args.assets)
    else:
        parser.error("Provide --release-json or both version and assets")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(content, encoding="utf-8")


if __name__ == "__main__":
    main()
