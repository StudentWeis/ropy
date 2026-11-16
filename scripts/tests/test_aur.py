"""Behavioral tests for verified AUR PKGBUILD generation."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "generate_aur_pkgbuild.py"


class AurTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("aur", SCRIPT)
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)

    def test_local_render_hashes_archive_icon_and_desktop(self):
        with tempfile.TemporaryDirectory() as directory:
            tmp = Path(directory)
            archive = tmp / "ropy-x86_64-unknown-linux-gnu.tar.xz"
            icon = tmp / "logo.png"
            desktop = tmp / "ropy.desktop"
            archive.write_bytes(b"archive-bytes")
            icon.write_bytes(b"icon-bytes")
            desktop.write_bytes(b"desktop-bytes")

            pkgbuild = self.module.render_local(
                "0.7.1", archive, icon_path=icon, desktop_path=desktop
            )
            self.assertIn("pkgver=0.7.1\n", pkgbuild)
            self.assertIn("pkgrel=1\n", pkgbuild)
            self.assertIn(self.module.sha256_file(archive), pkgbuild)
            self.assertIn(self.module.sha256_file(icon), pkgbuild)
            self.assertIn(self.module.sha256_file(desktop), pkgbuild)
            self.assertNotIn("SKIP", pkgbuild)

    def test_prerelease_and_injected_versions_fail(self):
        for version in ("0.7.1-beta.1", "latest", "0.7.1'", "v0.7.1"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.module.render_local(version, Path("unused"))

    def test_release_prerelease_and_missing_digest_fail(self):
        with self.assertRaises(ValueError):
            self.module.render_release({"isDraft": False, "isPrerelease": True})

        release = {
            "tagName": "0.7.1",
            "isDraft": False,
            "isPrerelease": False,
            "assets": [{"name": "ropy-x86_64-unknown-linux-gnu.tar.xz", "digest": None}],
        }
        with self.assertRaises(ValueError):
            self.module.render_release(release)

    def test_release_uses_published_digest_and_resolves_pkgrel(self):
        with tempfile.TemporaryDirectory() as directory:
            tmp = Path(directory)
            icon = tmp / "logo.png"
            desktop = tmp / "ropy.desktop"
            icon.write_bytes(b"icon-v1")
            desktop.write_bytes(b"desktop-v1")

            release = {
                "tagName": "0.7.1",
                "isDraft": False,
                "isPrerelease": False,
                "assets": [
                    {
                        "name": "ropy-x86_64-unknown-linux-gnu.tar.xz",
                        "digest": "sha256:" + "a" * 64,
                    }
                ],
            }

            first = self.module.render_release(
                release, icon_path=icon, desktop_path=desktop
            )
            self.assertIn("pkgrel=1\n", first)

            # Re-running with identical inputs keeps pkgrel=1 so git diff stays empty.
            unchanged = self.module.render_release(
                release, icon_path=icon, desktop_path=desktop, existing_pkgbuild=first
            )
            self.assertEqual(unchanged, first)

            # Packaging-only change on the same version bumps pkgrel.
            desktop.write_bytes(b"desktop-v2")
            bumped = self.module.render_release(
                release, icon_path=icon, desktop_path=desktop, existing_pkgbuild=first
            )
            self.assertIn("pkgrel=2\n", bumped)

            # New upstream version resets pkgrel to 1.
            next_release = {**release, "tagName": "0.7.2"}
            reset = self.module.render_release(
                next_release, icon_path=icon, desktop_path=desktop, existing_pkgbuild=bumped
            )
            self.assertIn("pkgver=0.7.2\n", reset)
            self.assertIn("pkgrel=1\n", reset)
