"""Generate a stable-release Arch Linux PKGBUILD for ropy-bin."""
import argparse
import hashlib
import json
from pathlib import Path
import re

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ICON = REPO_ROOT / "assets" / "logo.png"
DEFAULT_DESKTOP = REPO_ROOT / "packaging" / "aur" / "ropy-bin" / "ropy.desktop"
STABLE_VERSION_RE = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
SHA256_DIGEST_RE = re.compile(r"sha256:([0-9a-f]{64})")
PKGVER_LINE_RE = re.compile(r"^pkgver=(.+)$", re.MULTILINE)
PKGREL_LINE_RE = re.compile(r"^pkgrel=([1-9][0-9]*)$", re.MULTILINE)


def sha256_file(path: Path) -> str:
    """Return the lowercase hex SHA-256 digest of a file."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_stable_version(version: str) -> None:
    """Reject non-stable or malformed version strings."""
    if not STABLE_VERSION_RE.fullmatch(version):
        raise ValueError(f"Expected a stable release tag such as 0.7.1, got: {version!r}")


def parse_pkgbuild_version(content: str) -> tuple[str, int] | None:
    """Extract (pkgver, pkgrel) from an existing PKGBUILD, or None if missing."""
    ver_match = PKGVER_LINE_RE.search(content)
    rel_match = PKGREL_LINE_RE.search(content)
    if not ver_match or not rel_match:
        return None
    return ver_match.group(1).strip().strip("'\""), int(rel_match.group(1))


def render_pkgbuild(
    version: str,
    archive_sha256: str,
    icon_sha256: str,
    desktop_sha256: str,
    pkgrel: int = 1,
) -> str:
    """Render PKGBUILD contents for a verified stable release."""
    validate_stable_version(version)
    if pkgrel < 1:
        raise ValueError(f"pkgrel must be >= 1, got: {pkgrel}")
    for label, digest in (
        ("archive_sha256", archive_sha256),
        ("icon_sha256", icon_sha256),
        ("desktop_sha256", desktop_sha256),
    ):
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError(f"Invalid SHA-256 hex digest for {label}: {digest!r}")

    return f"""# Maintainer: Student Wei <studentweis@gmail.com>
pkgname=ropy-bin
pkgver={version}
pkgrel={pkgrel}
pkgdesc='A clipboard manager built with Rust and GPUI (prebuilt binary)'
arch=('x86_64')
url='https://github.com/StudentWeis/ropy'
license=('MIT')
depends=('glibc' 'gcc-libs' 'libxcb' 'libxkbcommon' 'libxkbcommon-x11' 'libglvnd' 'wayland')
optdepends=('xdg-desktop-portal: desktop-approved global shortcuts (requires a compatible backend)')
provides=("ropy=$pkgver")
conflicts=('ropy')
options=('!strip' '!debug')
source=("ropy-$pkgver.png::$url/raw/refs/tags/$pkgver/assets/logo.png"
        'ropy.desktop')
source_x86_64=("ropy-$pkgver-x86_64.tar.xz::$url/releases/download/$pkgver/ropy-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=('{icon_sha256}'
            '{desktop_sha256}')
sha256sums_x86_64=('{archive_sha256}')

package() {{
    local release_dir="$srcdir/ropy-$CARCH-unknown-linux-gnu"
    install -Dm755 "$release_dir/ropy" "$pkgdir/usr/bin/ropy"
    install -Dm644 "$release_dir/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 "$srcdir/ropy.desktop" "$pkgdir/usr/share/applications/ropy.desktop"
    install -Dm644 "$srcdir/ropy-$pkgver.png" "$pkgdir/usr/share/pixmaps/ropy.png"
}}
"""


def resolve_pkgrel(
    version: str,
    archive_sha256: str,
    icon_sha256: str,
    desktop_sha256: str,
    existing_pkgbuild: str | None = None,
    pkgrel_override: int | None = None,
) -> int:
    """Determine pkgrel: reset to 1 for a new version, or bump on same-version packaging edits."""
    if pkgrel_override is not None:
        if pkgrel_override < 1:
            raise ValueError(f"pkgrel must be >= 1, got: {pkgrel_override}")
        return pkgrel_override
    if not existing_pkgbuild:
        return 1
    parsed = parse_pkgbuild_version(existing_pkgbuild)
    if not parsed:
        return 1
    existing_ver, existing_rel = parsed
    if existing_ver != version:
        return 1
    candidate = render_pkgbuild(
        version=version,
        archive_sha256=archive_sha256,
        icon_sha256=icon_sha256,
        desktop_sha256=desktop_sha256,
        pkgrel=existing_rel,
    )
    if candidate == existing_pkgbuild:
        return existing_rel
    return existing_rel + 1


def extract_release_archive_digest(release: dict) -> tuple[str, str]:
    """Validate release metadata and return (version, x86_64_archive_sha256)."""
    if release.get("isDraft") or release.get("isPrerelease"):
        raise ValueError("Expected a published stable release")
    version = release["tagName"]
    validate_stable_version(version)
    name = "ropy-x86_64-unknown-linux-gnu.tar.xz"
    matches = [asset for asset in release.get("assets", []) if asset.get("name") == name]
    if len(matches) != 1:
        raise ValueError(f"Expected one release asset: {name}")
    digest = matches[0].get("digest") or ""
    match = SHA256_DIGEST_RE.fullmatch(digest)
    if not match:
        raise ValueError(f"Missing or invalid SHA-256 digest: {name}")
    return version, match.group(1)


def render_release(
    release: dict,
    icon_path: Path = DEFAULT_ICON,
    desktop_path: Path = DEFAULT_DESKTOP,
    existing_pkgbuild: str | None = None,
    pkgrel_override: int | None = None,
) -> str:
    """Render PKGBUILD from GitHub release JSON and local icon/desktop files."""
    version, archive_sha256 = extract_release_archive_digest(release)
    icon_sha256 = sha256_file(icon_path)
    desktop_sha256 = sha256_file(desktop_path)
    pkgrel = resolve_pkgrel(
        version=version,
        archive_sha256=archive_sha256,
        icon_sha256=icon_sha256,
        desktop_sha256=desktop_sha256,
        existing_pkgbuild=existing_pkgbuild,
        pkgrel_override=pkgrel_override,
    )
    return render_pkgbuild(
        version=version,
        archive_sha256=archive_sha256,
        icon_sha256=icon_sha256,
        desktop_sha256=desktop_sha256,
        pkgrel=pkgrel,
    )


def render_local(
    version: str,
    archive_path: Path,
    icon_path: Path = DEFAULT_ICON,
    desktop_path: Path = DEFAULT_DESKTOP,
    existing_pkgbuild: str | None = None,
    pkgrel_override: int | None = None,
) -> str:
    """Render PKGBUILD from a local x86_64 release archive and local icon/desktop files."""
    validate_stable_version(version)
    archive_sha256 = sha256_file(archive_path)
    icon_sha256 = sha256_file(icon_path)
    desktop_sha256 = sha256_file(desktop_path)
    pkgrel = resolve_pkgrel(
        version=version,
        archive_sha256=archive_sha256,
        icon_sha256=icon_sha256,
        desktop_sha256=desktop_sha256,
        existing_pkgbuild=existing_pkgbuild,
        pkgrel_override=pkgrel_override,
    )
    return render_pkgbuild(
        version=version,
        archive_sha256=archive_sha256,
        icon_sha256=icon_sha256,
        desktop_sha256=desktop_sha256,
        pkgrel=pkgrel,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-json", type=Path)
    parser.add_argument("--icon", type=Path, default=DEFAULT_ICON)
    parser.add_argument("--desktop", type=Path, default=DEFAULT_DESKTOP)
    parser.add_argument("--existing-pkgbuild", type=Path)
    parser.add_argument("--pkgrel", type=int)
    parser.add_argument("version", nargs="?")
    parser.add_argument("archive", nargs="?", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    existing = (
        args.existing_pkgbuild.read_text(encoding="utf-8")
        if args.existing_pkgbuild and args.existing_pkgbuild.is_file()
        else None
    )

    if args.release_json:
        content = render_release(
            json.loads(args.release_json.read_text(encoding="utf-8")),
            icon_path=args.icon,
            desktop_path=args.desktop,
            existing_pkgbuild=existing,
            pkgrel_override=args.pkgrel,
        )
    elif args.version and args.archive:
        content = render_local(
            args.version,
            args.archive,
            icon_path=args.icon,
            desktop_path=args.desktop,
            existing_pkgbuild=existing,
            pkgrel_override=args.pkgrel,
        )
    else:
        parser.error("Provide --release-json or both version and archive")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(content, encoding="utf-8")


if __name__ == "__main__":
    main()
