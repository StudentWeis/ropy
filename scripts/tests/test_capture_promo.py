"""Verify capture failures terminate only the child started by the script."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location(
    "capture_promo", Path(__file__).resolve().parents[1] / "capture_promo.py"
)
promo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(promo)


class CapturePromoTests(unittest.TestCase):
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
