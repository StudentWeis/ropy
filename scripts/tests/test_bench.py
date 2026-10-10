"""Benchmark reports must not imply comparisons across incompatible runs."""
import importlib.util
import json
from unittest.mock import Mock, patch
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("bench", Path(__file__).parents[1] / "bench.py")
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)


class BenchmarkTests(unittest.TestCase):
    def result(self):
        return {"schema_version": 1, "suite_version": 1, "fixture_version": 1,
                "environment": {"os": "Darwin", "os_version": "26", "arch": "arm64",
                                "machine": "bench-mac", "cpu": "M1", "rustc": "1.99",
                                "profile": "release/bench", "rustflags": ""},
                "version": "0.5.7", "commit": "abc", "dirty": False,
                "metrics": {"insert_ns": {"value": 100, "unit": "ns"}}}

    def test_report_same_environment_calculates_delta(self):
        old, new = self.result(), self.result()
        new["metrics"]["insert_ns"]["value"] = 90
        self.assertIn("-10.0%", bench.report(new, old))

    def test_report_changed_environment_rejects_performance_delta(self):
        for key in self.result()["environment"]:
            old, new = self.result(), self.result()
            new["environment"][key] = "different"
            with self.subTest(key=key):
                self.assertIn("not comparable", bench.report(new, old))
                self.assertNotIn("+0.0%", bench.report(new, old))

    def test_report_changed_fixture_rejects_delta(self):
        old, new = self.result(), self.result()
        new["fixture_version"] = 2
        self.assertIn("not comparable", bench.report(new, old))

    def test_report_missing_metric_is_explicit(self):
        old, new = self.result(), self.result()
        new["metrics"] = {}
        self.assertIn("not measured", bench.report(new, old))

    def test_build_binary_uses_cargo_reported_target_path(self):
        message = {"reason": "compiler-artifact", "target": {"name": "ropy"},
                   "executable": "/custom-target/aarch64-apple-darwin/release/ropy"}
        completed = Mock(stdout=json.dumps(message))
        with patch.object(bench.subprocess, "run", return_value=completed):
            self.assertEqual(bench.build_binary(), Path(message["executable"]))
        completed.check_returncode.assert_called_once()

    def test_report_different_package_format_rejects_delta(self):
        old, new = self.result(), self.result()
        old["metrics"]["package_bytes"] = {"value": 100, "unit": "bytes", "context": {"format": ".zip"}}
        new["metrics"]["package_bytes"] = {"value": 80, "unit": "bytes", "context": {"format": ".tar.xz"}}
        package_row = next(line for line in bench.report(new, old).splitlines() if "| package_bytes |" in line)
        self.assertIn("not comparable", package_row)

    def test_memory_sampling_failure_terminates_only_launched_child(self):
        child = Mock(pid=12345)
        child.poll.return_value = None

        def launch(arguments, **kwargs):
            Path(arguments[-1]).write_text(json.dumps({"stored_records": 200, "loaded_records": 100}))
            return child

        with patch.object(bench.platform, "system", return_value="Darwin"), \
             patch.object(bench.subprocess, "Popen", side_effect=launch), \
             patch.object(bench.time, "sleep"), \
             patch.object(bench, "output", side_effect=RuntimeError("ps failed")):
            with self.assertRaisesRegex(RuntimeError, "ps failed"):
                bench.memory_samples(Path("/fake/ropy"))
        child.terminate.assert_called_once()
        child.wait.assert_called_once_with(timeout=5)
        child.kill.assert_not_called()

    def test_memory_samples_convert_kib_and_keep_raw_values(self):
        child = Mock(pid=12345)
        child.poll.return_value = None

        def launch(arguments, **kwargs):
            Path(arguments[-1]).write_text(json.dumps({"stored_records": 200, "loaded_records": 100}))
            return child

        with patch.object(bench.platform, "system", return_value="Darwin"), \
             patch.object(bench.subprocess, "Popen", side_effect=launch), \
             patch.object(bench.time, "sleep"), \
             patch.object(bench, "output", return_value="1024"):
            result = bench.memory_samples(Path("/fake/ropy"))
        self.assertEqual(result["value"], 1024 * 1024)
        self.assertEqual(len(result["samples"]), 30)
        child.terminate.assert_called_once()

    def test_release_sizes_measure_archive_and_binary_without_extraction(self):
        import io
        import tarfile
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "ropy-aarch64-apple-darwin.tar.xz"
            with tarfile.open(archive, "w:xz") as output:
                info = tarfile.TarInfo("ropy-aarch64-apple-darwin/ropy")
                info.size = 123
                output.addfile(info, io.BytesIO(b"a" * 123))
            result = bench.release_sizes(root, "0.5.7")
            self.assertEqual(result["artifacts"][0]["binary_bytes"], 123)
            self.assertEqual(result["artifacts"][0]["package_bytes"], archive.stat().st_size)
            self.assertEqual(result["artifacts"][0]["target"], "aarch64-apple-darwin")


if __name__ == "__main__":
    unittest.main()
