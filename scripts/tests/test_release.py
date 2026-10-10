"""Exercise release packaging without compiling Ropy or publishing a release."""

import hashlib
import tarfile
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ReleaseTests(unittest.TestCase):
    def test_bundle_macos_settings_use_supported_schema(self):
        with (ROOT / "Cargo.toml").open("rb") as manifest:
            bundle = tomllib.load(manifest)["package"]["metadata"]["bundle"]
        self.assertNotIn("osx_minimum_system_version", bundle)
        self.assertEqual(bundle["macos"]["minimum_system_version"], "10.15")

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

    def test_release_preparation_confirmation_controls_mutations_and_exit(self):
        scripts = self.work / "scripts"
        scripts.mkdir()
        shutil.copyfile(ROOT / "scripts/release_prepare.sh", scripts / "release_prepare.sh")
        manifest = self.work / "Cargo.toml"
        original = '[package]\nversion = "0.1.0"\n[package.metadata.bundle.bin.ropy]\nversion = "0.1.0"\n'
        for name in ("precheck.sh", "record_build_size.sh"):
            path = scripts / name
            path.write_text('#!/bin/sh\nprintf "%s\\n" preparation >> steps\n')
            path.chmod(0o755)
        self.command("git", 'printf "%s\\n" changelog >> steps\n')
        self.command("dist", 'printf "%s\\n" plan >> steps\n')
        env = dict(self.env, NEW_VERSION="0.1.1", GITHUB_TOKEN="fixture", DRY_RUN="false")
        for answer in ("n", "N", "", "y", "\n"):
            with self.subTest(answer=answer):
                manifest.write_text(original)
                (self.work / "CHANGELOG.md").write_text("original changelog\n")
                (self.work / "steps").unlink(missing_ok=True)
                result = subprocess.run(
                    ["bash", str(scripts / "release_prepare.sh")], cwd=self.work,
                    env=env, input=answer, capture_output=True, text=True,
                )
                if answer in ("y", "\n"):
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn('version = "0.1.1"', manifest.read_text())
                    self.assertEqual((self.work / "steps").read_text().splitlines(),
                                     ["preparation", "preparation", "changelog", "plan"])
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(manifest.read_text(), original)
                    self.assertEqual((self.work / "CHANGELOG.md").read_text(), "original changelog\n")
                    self.assertFalse((self.work / "steps").exists())

    def test_version_dry_run_selects_only_the_desktop_package(self):
        self.command("cargo", "printf '%s\\n' \"$@\"\n")
        result = subprocess.run(
            ["bash", str(ROOT / "scripts/update_version.sh"), "patch"],
            cwd=self.work, env=self.env, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        args = result.stdout.splitlines()
        self.assertIn("--package", args)
        self.assertEqual(args[args.index("--package") + 1], "ropy")
        self.assertIn("--no-verify", args)
        self.assertNotIn("--execute", args)

    def test_dmg_selected_target_ignores_other_architecture(self):
        for target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            with self.subTest(target=target):
                other = self.work / "target/stale/release/bundle/osx/Ropy.app"
                other.mkdir(parents=True, exist_ok=True)
                (other / "binary").write_text("wrong architecture")
                self.command("cargo", '''
[[ "$1" == bundle && "$2" == --package && "$3" == ropy ]]
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

    def test_bundle_update_archive_contains_complete_application_and_checksum(self):
        self.command("cargo", '''
mkdir -p target/aarch64-apple-darwin/release/bundle/osx/Ropy.app/Contents/MacOS
printf binary > target/aarch64-apple-darwin/release/bundle/osx/Ropy.app/Contents/MacOS/ropy
printf metadata > target/aarch64-apple-darwin/release/bundle/osx/Ropy.app/Contents/Info.plist
''')
        self.command("hdiutil", "exit 0\n")
        result = self.bundle("aarch64-apple-darwin")
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.work / "target/distrib/ropy-aarch64-apple-darwin-app.tar.xz"
        with tarfile.open(archive) as payload:
            self.assertEqual(payload.extractfile("Ropy.app/Contents/Info.plist").read(), b"metadata")
            self.assertEqual(payload.extractfile("Ropy.app/Contents/MacOS/ropy").read(), b"binary")
        checksum = Path(str(archive) + ".sha256").read_text().split()[0]
        self.assertEqual(checksum, hashlib.sha256(archive.read_bytes()).hexdigest())

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
