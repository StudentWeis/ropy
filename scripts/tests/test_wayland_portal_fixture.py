"""Fixture failures must report and stop without needing a live portal or desktop."""

from contextlib import contextmanager
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import Mock, patch


SCRIPT = Path(__file__).with_name("wayland_portal_fixture.py")


class PortalFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.binary = self.work / "ropy"
        self.binary.write_text("not an executable")
        self.environment = self.work / "session.env"
        self.environment.write_text("")
        self.output = self.work / "output"
        self.stopped = threading.Event()
        self.worker_errors = []

        class Object:
            def __init__(self, *args):
                pass

        service = ModuleType("dbus.service")
        service.Object = Object
        service.BusName = Mock()
        service.method = service.signal = lambda *args, **kwargs: lambda function: function
        dbus = ModuleType("dbus")
        dbus.service = service
        dbus.SessionBus = Mock(return_value=SimpleNamespace(add_signal_receiver=Mock()))
        mainloop = ModuleType("dbus.mainloop.glib")
        mainloop.DBusGMainLoop = Mock()
        repository = ModuleType("gi.repository")
        repository.GLib = SimpleNamespace(
            MainLoop=lambda: SimpleNamespace(run=self.wait_for_exit, quit=self.stopped.set),
            idle_add=lambda function: function(),
        )
        modules = {
            "dbus": dbus, "dbus.service": service,
            "dbus.mainloop": ModuleType("dbus.mainloop"), "dbus.mainloop.glib": mainloop,
            "gi": ModuleType("gi"), "gi.repository": repository,
        }
        spec = importlib.util.spec_from_file_location("portal_fixture", SCRIPT)
        self.fixture = importlib.util.module_from_spec(spec)
        # The imported fixture retains these inert adapters; other tests keep their modules.
        with patch.dict(sys.modules, modules):
            spec.loader.exec_module(self.fixture)

    def wait_for_exit(self):
        if not self.stopped.wait(timeout=2):
            raise TimeoutError("fixture worker did not stop the main loop")

    def run_fixture(self):
        arguments = [str(SCRIPT), "--binary", str(self.binary),
                     "--environment", str(self.environment), "--input-helper", "unused",
                     "--output", str(self.output), "--mode", "accept"]
        with patch.object(sys, "argv", arguments), patch("builtins.print") as output, \
             patch.object(threading, "excepthook", side_effect=self.worker_errors.append):
            code = self.fixture.main()
        self.assertEqual(code, 1)
        self.assertTrue(self.stopped.is_set())
        self.assertEqual(self.worker_errors, [])
        report = json.loads(output.call_args.args[0])
        self.assertEqual(report["status"], "failed")
        return report

    def test_portal_fixture_relative_output_passes_absolute_isolated_config(self):
        self.environment.write_text("DISPLAY=:1\nWAYLAND_DISPLAY=wayland-fixture\nDBUS_SESSION_BUS_ADDRESS=wrong\n")
        self.output = Path(os.path.relpath(self.output))
        child = Mock()
        child.poll.return_value = 0
        with patch.object(self.fixture.subprocess, "Popen", return_value=child) as launch, \
             patch.object(self.fixture.time, "sleep", side_effect=RuntimeError("scenario stopped")), \
             patch.dict(os.environ, {"DBUS_SESSION_BUS_ADDRESS": "private-bus"}):
            self.run_fixture()
        env = launch.call_args.kwargs["env"]
        self.assertEqual(env["XDG_CONFIG_HOME"], str((self.work / "output/config").resolve()))
        self.assertTrue((Path(env["XDG_CONFIG_HOME"]) / "ropy/config.toml").is_file())
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "private-bus")
        self.assertEqual(env["WAYLAND_DISPLAY"], "wayland-fixture")
        self.assertNotIn("DISPLAY", env)

    def test_portal_fixture_failed_launch_reports_and_exits(self):
        report = self.run_fixture()
        self.assertIn(str(self.binary), report["error"])
        self.assertEqual(json.loads((self.output / "report.json").read_text()), report)

    def test_portal_fixture_failed_log_open_reports_and_exits(self):
        (self.output / "application.log").mkdir(parents=True)
        report = self.run_fixture()
        self.assertIn("application.log", report["error"])
        self.assertEqual(json.loads((self.output / "report.json").read_text()), report)

    def test_portal_fixture_failed_cleanup_still_reports_and_exits(self):
        child = Mock()
        child.poll.return_value = None
        child.terminate.side_effect = OSError("termination failed")
        with patch.object(self.fixture.subprocess, "Popen", return_value=child), \
             patch.object(self.fixture.time, "sleep", side_effect=RuntimeError("scenario failed")):
            report = self.run_fixture()
        self.assertIn("termination failed", report["cleanup_error"])
        self.assertEqual(json.loads((self.output / "report.json").read_text()), report)

    def test_portal_fixture_failed_log_close_still_reports_and_exits(self):
        original_open = Path.open

        @contextmanager
        def failing_log(path, *args, **kwargs):
            with original_open(path, *args, **kwargs) as log:
                yield log
            raise OSError("log close failed")

        def open_file(path, *args, **kwargs):
            if path.name == "application.log":
                return failing_log(path, *args, **kwargs)
            return original_open(path, *args, **kwargs)

        with patch.object(Path, "open", open_file):
            report = self.run_fixture()
        self.assertIn("log close failed", report["error"])
        self.assertEqual(json.loads((self.output / "report.json").read_text()), report)

    def test_portal_fixture_failed_report_write_still_exits(self):
        (self.output / "report.json").mkdir(parents=True)
        report = self.run_fixture()
        self.assertIn("report.json", report["report_error"])


if __name__ == "__main__":
    unittest.main()
