"""Verify capture failures terminate only the child started by the script."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location(
    "capture_promo", Path(__file__).resolve().parents[1] / "capture_promo.py"
)
promo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(promo)


class CapturePromoTests(unittest.TestCase):
    def test_capture_main_uses_reported_artifact_or_explicit_binary(self):
        for explicit in (False, True):
            with self.subTest(explicit=explicit), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                stale = root / "target/debug/ropy"
                stale.parent.mkdir(parents=True)
                stale.write_bytes(b"stale")
                binary = root / "custom-target/aarch64-apple-darwin/debug/ropy"
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b"current")
                message = {"reason": "compiler-artifact", "target": {"name": "ropy", "kind": ["bin"]},
                           "executable": str(binary)}
                def run(args, **kwargs):
                    return subprocess.CompletedProcess(args, 0, stdout=json.dumps(message))
                args = ["capture_promo.py", "--output", str(root / "output")]
                if explicit:
                    args += ["--binary", str(binary)]
                with patch.object(promo, "ROOT", root), \
                     patch.object(promo.platform, "system", return_value="Darwin"), \
                     patch.object(promo.subprocess, "run", side_effect=run) as commands, \
                     patch.object(promo.subprocess, "check_output", return_value=json.dumps({"target_directory": str(root / "target")})), \
                     patch.object(promo, "capture_theme", return_value=root / "image.png") as capture, \
                     patch.object(sys, "argv", args), patch("builtins.print"):
                    promo.main()
                self.assertEqual(capture.call_count, 4)
                self.assertTrue(all(call.args[0] == binary.resolve() for call in capture.call_args_list))
                self.assertEqual(commands.call_args_list[-1].args[0][0], str(binary.resolve()))
                builds = [call for call in commands.call_args_list if call.args[0][0] == "cargo"]
                self.assertEqual(len(builds), 0 if explicit else 1)

    def test_capture_build_failure_or_missing_artifact_never_starts_capture(self):
        for status in (0, 7):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                stale = root / "target/debug/ropy"
                stale.parent.mkdir(parents=True)
                stale.write_bytes(b"stale")
                def run(args, **kwargs):
                    result = subprocess.CompletedProcess(args, status if args[0] == "cargo" else 0,
                                                         stdout="")
                    if kwargs.get("check"):
                        result.check_returncode()
                    return result
                with patch.object(promo, "ROOT", root), \
                     patch.object(promo.platform, "system", return_value="Darwin"), \
                     patch.object(promo.subprocess, "run", side_effect=run), \
                     patch.object(promo.subprocess, "check_output", return_value=json.dumps({"target_directory": str(root / "target")})), \
                     patch.object(promo, "capture_theme") as capture, \
                     patch.object(sys, "argv", ["capture_promo.py", "--output", str(root / "output")]), \
                     patch("builtins.print"):
                    with self.assertRaises((RuntimeError, subprocess.CalledProcessError)):
                        promo.main()
                capture.assert_not_called()

    def test_capture_timeout_terminates_child(self):
        child = Mock(pid=123)
        child.poll.return_value = None
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(promo.subprocess, "Popen", return_value=child):
                with self.assertRaises(TimeoutError):
                    promo.capture_theme(Path("ropy"), Path("helper"), "ropy-light", "en", Path(directory), timeout=0)
        child.terminate.assert_called_once()
        child.wait.assert_called_once_with(timeout=5)

    def test_capture_failed_screenshot_terminates_child(self):
        child = Mock(pid=123)
        child.poll.return_value = None
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(promo.subprocess, "Popen", return_value=child), \
                 patch.object(promo, "wait_ready"), \
                 patch.object(promo.subprocess, "check_output", return_value="456"), \
                 patch.object(promo.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "screencapture")):
                with self.assertRaises(subprocess.CalledProcessError):
                    promo.capture_theme(Path("ropy"), Path("helper"), "ropy-light", "en", Path(directory))
        child.terminate.assert_called_once()

    def test_stop_child_unresponsive_killed_and_reaped(self):
        child = Mock()
        child.poll.return_value = None
        child.wait.side_effect = [subprocess.TimeoutExpired("ropy", 5), 0]
        promo.stop_child(child)
        child.kill.assert_called_once()
        self.assertEqual(child.wait.call_count, 2)

    def test_wait_ready_exited_child_fails_without_waiting(self):
        child = Mock()
        child.poll.return_value = 1
        with self.assertRaises(RuntimeError):
            promo.wait_ready(child, Path("missing"), float("inf"))

    def test_capture_waits_for_stable_frames_and_removes_temporary_data(self):
        child = Mock(pid=123)
        child.poll.return_value = None
        frames = iter([b"first", b"settled", b"settled", b"settled", b"settled"])
        def screenshot(args, **kwargs):
            Path(args[-1]).write_bytes(next(frames))
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            with patch.object(promo.subprocess, "Popen", return_value=child), \
                 patch.object(promo, "wait_ready"), \
                 patch.object(promo.subprocess, "check_output", return_value="456") as lookup, \
                 patch.object(promo.subprocess, "run", side_effect=screenshot) as capture, \
                 patch.object(promo.time, "sleep"):
                result = promo.capture_theme(Path("ropy"), Path("helper"), "ropy-light", "en", work)
            self.assertEqual(result.read_bytes(), b"settled")
            self.assertEqual(capture.call_count, 5)
            self.assertTrue(all(call.args[0][3] == "-l456" for call in capture.call_args_list))
            self.assertTrue(all(call.args[0] == ["swift", "helper", "123"] for call in lookup.call_args_list))
            self.assertFalse(any(path.is_dir() for path in work.iterdir()))
        child.terminate.assert_called_once()
