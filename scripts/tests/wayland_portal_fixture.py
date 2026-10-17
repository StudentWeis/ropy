#!/usr/bin/env python3
"""Exercise Ropy against a fake portal on an isolated D-Bus session and live Sway.

Run with dbus-run-session. Requires python3-dbus, python3-gi, swaymsg and an
isolated compositor. The input helper must accept a chord such as alt+F4.
This verifies the portal client contract, not a desktop portal implementation.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import time

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

PORTAL = "org.freedesktop.portal.GlobalShortcuts"
ROOT = "/org/freedesktop/portal/desktop"
TOKEN = "ropy-fixture-activation-token"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--environment", type=Path, required=True)
    parser.add_argument("--input-helper", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--mode", choices=["accept", "deny", "pending", "startup"], required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    for line in args.environment.read_text().splitlines():
        key, separator, value = line.partition("=")
        if separator and key != "DBUS_SESSION_BUS_ADDRESS":
            env[key] = value
    env.pop("DISPLAY", None)
    config = args.output / "config"
    (config / "ropy").mkdir(parents=True, exist_ok=True)
    (config / "ropy/config.toml").write_text(
        'language="en"\n[autostart]\nenabled=false\n[update]\nauto_check=false\n'
    )
    env.update(XDG_CONFIG_HOME=str(config), WAYLAND_DEBUG="client")
    DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    name = dbus.service.BusName("org.freedesktop.portal.Desktop", bus)
    loop = GLib.MainLoop()
    state = {"bound": False, "closed": False, "disconnected": False}
    objects = []

    class Properties(dbus.service.Object):
        @dbus.service.method("org.freedesktop.DBus.Properties", in_signature="ss", out_signature="v")
        def Get(self, interface, property_name):
            return dbus.UInt32(1)

        @dbus.service.method("org.freedesktop.DBus.Properties", in_signature="s", out_signature="a{sv}")
        def GetAll(self, interface):
            return {"version": dbus.UInt32(1)}

    class Request(Properties):
        @dbus.service.signal("org.freedesktop.portal.Request", signature="ua{sv}")
        def Response(self, code, results):
            pass

    class Session(Properties):
        @dbus.service.method("org.freedesktop.portal.Session", in_signature="", out_signature="")
        def Close(self):
            state["closed"] = True

    def request(sender, options, code, results, respond=True):
        path = f'{ROOT}/request/{sender[1:].replace(".", "_")}/{options["handle_token"]}'
        obj = Request(name, path)
        objects.append(obj)
        if respond:
            GLib.idle_add(lambda: (obj.Response(code, results), False)[1])
        return dbus.ObjectPath(path)

    class Portal(Properties):
        @dbus.service.method(PORTAL, in_signature="a{sv}", out_signature="o", sender_keyword="sender")
        def CreateSession(self, options, sender=None):
            path = f'{ROOT}/session/{sender[1:].replace(".", "_")}/{options["session_handle_token"]}'
            objects.append(Session(name, path))
            state.update(session=path, sender=sender)
            return request(sender, options, 0, {"session_handle": dbus.String(path)})

        @dbus.service.method(PORTAL, in_signature="oa(sa{sv})sa{sv}", out_signature="o", sender_keyword="sender")
        def BindShortcuts(self, session, shortcuts, parent, options, sender=None):
            state["bound"] = True
            state["trigger"] = str(shortcuts[0][1].get("preferred_trigger"))
            state["description"] = str(shortcuts[0][1].get("description"))
            response = {"shortcuts": dbus.Array([
                dbus.Struct(("show", dbus.Dictionary({"description": "Ropy", "trigger_description": "Ctrl+Shift+D"}, signature="sv")))
            ], signature="(sa{sv})")}
            return request(sender, options, 1 if args.mode == "deny" else 0,
                           response, respond=args.mode != "pending")

        @dbus.service.signal(PORTAL, signature="osta{sv}")
        def Activated(self, session, shortcut, timestamp, options):
            pass

    portal = Portal(name, ROOT)

    def owner_changed(name, old, new):
        if name == state.get("sender") and old and not new:
            state["disconnected"] = True

    bus.add_signal_receiver(owner_changed, signal_name="NameOwnerChanged", dbus_interface="org.freedesktop.DBus")
    report = {"mode": args.mode, "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(), "checks": []}

    def exercise(app):
        def wait(predicate, label, timeout=8):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if predicate():
                    report["checks"].append(label)
                    return
                time.sleep(0.05)
            raise AssertionError(label)

        def popup():
            tree = json.loads(subprocess.check_output(["swaymsg", "-t", "get_tree"], env=env))
            def nodes(node):
                yield node
                for child in node.get("nodes", []) + node.get("floating_nodes", []):
                    yield from nodes(child)
            return next((node for node in nodes(tree) if node.get("pid") == app.pid and node.get("visible")), None)

        def key(chord):
            subprocess.run(["sudo", "python3", str(args.input_helper), chord], check=True, env=env)

        wait(lambda: state["bound"], "portal received binding request")
        assert state["trigger"] == "CTRL+SHIFT+D", state
        assert state["description"], state
        time.sleep(0.5)
        if args.mode != "startup":
            assert popup() is None, "hidden startup"
        if args.mode == "accept":
            def emit(session, shortcut):
                def signal():
                    portal.Activated(dbus.ObjectPath(session), shortcut, dbus.UInt64(0), {"activation_token": TOKEN})
                    return False
                GLib.idle_add(signal)
            emit(state["session"] + "_other", "show")
            emit(state["session"], "unrelated")
            time.sleep(0.3)
            assert popup() is None, "unrelated portal events must not open popup"
            report["checks"].append("unrelated session and shortcut signals ignored")
            emit(state["session"], "show")
            wait(lambda: popup() is not None, "portal signal opens native popup")
            wait(lambda: f'activate("{TOKEN}"' in (args.output / "application.log").read_text(), "activation token reaches Wayland compositor")
            key("Escape")
            wait(lambda: popup() is None, "portal popup dismisses")
            subprocess.run([str(args.binary), "--show"], env=env | {"XDG_ACTIVATION_TOKEN": TOKEN + "-ipc"}, check=True, timeout=5)
            wait(lambda: popup() is not None, "second process reopens resident instance")
            wait(lambda: f'activate("{TOKEN}-ipc"' in (args.output / "application.log").read_text(), "second process token reaches compositor")
        elif args.mode == "startup":
            wait(lambda: popup() is not None, "--show first launch maps popup")
            protocol = (args.output / "application.log").read_text()
            assert protocol.count(f'activate("{TOKEN}-startup"') == 1, "one-shot startup token used once"
            assert protocol.count(".get_toplevel(") == 1, "startup surface must not be destroyed and recreated"
            report["checks"].append("initial surface retains one-shot startup token")
        else:
            if args.mode == "deny":
                wait(lambda: state["disconnected"], "denied registration releases private bus connection")
            subprocess.run([str(args.binary), "--show"], env=env, check=True, timeout=5)
            wait(lambda: popup() is not None, "manual activation remains available")
        wait(lambda: popup() and popup()["focused"], "popup has input focus")
        key("alt+F4")
        wait(lambda: app.poll() is not None, "normal quit completes while portal is active or pending")
        assert app.returncode == 0, app.returncode
        wait(lambda: state["disconnected"], "portal connection released on shutdown")
        if args.mode in ("accept", "startup"):
            assert state["closed"], "active portal session explicitly closed"
            report["checks"].append("active session explicitly closed before quit")
        report.update(status="pass", session_closed=state["closed"])

    def run():
        try:
            with (args.output / "application.log").open("w") as log:
                app = None
                try:
                    command = [str(args.binary)] + (["--show"] if args.mode == "startup" else [])
                    app_env = env | ({"XDG_ACTIVATION_TOKEN": TOKEN + "-startup"} if args.mode == "startup" else {})
                    app = subprocess.Popen(command, env=app_env, stdout=log, stderr=subprocess.STDOUT)
                    exercise(app)
                except Exception as error:
                    report.update(status="failed", error=str(error))
                finally:
                    try:
                        if app is not None and app.poll() is None:
                            app.terminate()
                            try:
                                app.wait(timeout=5)
                            except subprocess.TimeoutExpired:
                                app.kill()
                                app.wait(timeout=5)
                    except Exception as error:
                        report.update(status="failed", cleanup_error=str(error))
        except Exception as error:
            report.update(status="failed", error=str(error))
        finally:
            try:
                (args.output / "report.json").write_text(json.dumps(report, indent=2))
            except Exception as error:
                report.update(status="failed", report_error=str(error))
            finally:
                GLib.idle_add(loop.quit)

    thread = threading.Thread(target=run)
    thread.start()
    loop.run()
    thread.join()
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
