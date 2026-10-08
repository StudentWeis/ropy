"""Exercise release packaging without compiling Ropy or publishing a release."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.bin = self.work / "bin"
        self.bin.mkdir()
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}")

    def command(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/bash\nset -eu\n" + body)
        path.chmod(0o755)

    def bundle(self, target):
        return subprocess.run(
            ["bash", str(ROOT / "scripts/build_macos_dmg.sh"), target],
            cwd=self.work, env=self.env, capture_output=True, text=True,
        )

    def test_dmg_selected_target_ignores_other_architecture(self):
        for target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            with self.subTest(target=target):
                other = self.work / "target/stale/release/bundle/osx/Ropy.app"
                other.mkdir(parents=True, exist_ok=True)
                (other / "binary").write_text("wrong architecture")
                self.command("cargo", '''
while [[ "$1" != "--target" ]]; do shift; done
bundle="target/$2/release/bundle/osx/Ropy.app"
mkdir -p "$bundle"
printf '%s' "$2" > "$bundle/binary"
''')
                self.command("hdiutil", '''
while [[ "$1" != "-srcfolder" ]]; do shift; done
stage="$2"
[[ "$(readlink "$stage/Applications")" == /Applications ]]
printf '%s' "$stage" > stage-path
cp "$stage/Ropy.app/binary" "${@: -1}"
''')
                result = self.bundle(target)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    (self.work / f"target/distrib/ropy-{target}.dmg").read_text(), target
                )
                self.assertFalse(Path((self.work / "stage-path").read_text()).exists())

    def test_dmg_missing_bundle_fails_without_using_stale_output(self):
        other = self.work / "target/stale/release/bundle/osx/Ropy.app"
        other.mkdir(parents=True)
        self.command("cargo", "exit 0\n")
        result = self.bundle("aarch64-apple-darwin")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Ropy.app", result.stderr)

    def test_dmg_image_failure_removes_staging_directory(self):
        self.command("cargo", '''
mkdir -p target/aarch64-apple-darwin/release/bundle/osx/Ropy.app
''')
        self.command("hdiutil", '''
while [[ "$1" != "-srcfolder" ]]; do shift; done
printf '%s' "$2" > stage-path
exit 7
''')
        result = self.bundle("aarch64-apple-darwin")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertFalse(Path((self.work / "stage-path").read_text()).exists())

    def manifest(self):
        notes = self.work / "notes.txt"
        notes.write_text('Release "notes"\n第二行\n')
        return subprocess.run(
            ["bash", str(ROOT / "scripts/generate_update_manifest.sh"),
             str(self.work), "StudentWeis/ropy", "v0.5.5", str(notes),
             str(self.work / "latest.json")], capture_output=True, text=True,
        )

    def test_manifest_archives_preserve_updater_contract(self):
        names = ["ropy-aarch64-apple-darwin.tar.xz", "ropy-x86_64-pc-windows-msvc.zip"]
        for name in names:
            (self.work / name).write_bytes(b"archive")
            (self.work / f"{name}.sha256").write_text("checksum")
        (self.work / "ropy-aarch64-apple-darwin.dmg").write_bytes(b"dmg")
        result = self.manifest()
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((self.work / "latest.json").read_text())
        self.assertEqual(manifest["tag_name"], "v0.5.5")
        self.assertEqual(manifest["body"], 'Release "notes"\n第二行\n')
        self.assertEqual(len(manifest["assets"]), 4)
        for asset in manifest["assets"]:
            self.assertEqual(asset["size"], (self.work / asset["name"]).stat().st_size)
            self.assertEqual(asset["browser_download_url"],
                             f"https://github.com/StudentWeis/ropy/releases/download/v0.5.5/{asset['name']}")

    def test_manifest_missing_checksum_fails(self):
        (self.work / "ropy-aarch64-apple-darwin.tar.xz").write_bytes(b"archive")
        result = self.manifest()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing checksum", result.stderr)

    def test_manifest_missing_archives_fails(self):
        result = self.manifest()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No update archives", result.stderr)


if __name__ == "__main__":
    unittest.main()
