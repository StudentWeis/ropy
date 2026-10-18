#!/usr/bin/env python3
"""Run only inside an isolated Hyprland session with disposable XDG directories.

Usage: hyprland_popup_fixture.py /absolute/path/to/ropy
Requires wtype, foot and Hyprland 0.55+; never run on the user's desktop.
"""

import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time


def request(command):
    path = (Path(os.environ["XDG_RUNTIME_DIR"]) / "hypr"
            / os.environ["HYPRLAND_INSTANCE_SIGNATURE"] / ".socket.sock")
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(2)
        stream.connect(str(path))
        stream.sendall(command.encode())
        chunks = []
        while data := stream.recv(65536):
            chunks.append(data)
        return b"".join(chunks).decode()


def wait_for(predicate):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if result := predicate():
            return result
        time.sleep(0.05)
    raise AssertionError("timed out waiting for window state")


def main():
    binary = str(Path(sys.argv[1]).resolve())
    assert not os.environ.get("DISPLAY"), "unset DISPLAY for native Wayland"
    peer = subprocess.Popen(["foot", "--app-id=ropy-test-peer", "sleep", "120"])
    app = None

    def clients():
        return [c for c in json.loads(request("j/clients")) if c["pid"] == app.pid]

    def key(*args):
        subprocess.run(["wtype", "-m", "logo", "-m", "alt", "-m", "ctrl", "-m", "shift", *args], check=True)

    try:
        app = subprocess.Popen([binary])
        time.sleep(2)
        assert app.poll() is None
        assert not clients(), "startup must remain hidden"
        peer_window = wait_for(lambda: next(
            (c for c in json.loads(request("j/clients")) if c["pid"] == peer.pid), None))
        for cycle in range(3):
            if cycle == 1:
                assert request("/reload").strip() == "ok"
                time.sleep(0.3)
            if cycle == 2:
                # Exercise the registered event route. The separate live Rust
                # shortcut test verifies physical chord -> callback delivery;
                # wtype's synthetic letter keymap is unsuitable for that check.
                response = request(
                    "/eval local count = 0; for _, entry in pairs(_ropy_hotkeys or {}) do "
                    "if entry.bind:is_enabled() then count = count + 1; "
                    "hl.dispatch(hl.dsp.event(entry.token)); end; end; assert(count == 1)"
                )
                assert response.strip() == "ok", response
            else:
                subprocess.run([binary, "--show"], check=True)
            client = wait_for(lambda: next(iter(clients()), None))
            assert not client["xwayland"], client
            assert client["floating"], client
            assert client["size"] == [400, 550], client
            monitor = next(m for m in json.loads(request("j/monitors"))
                           if m["id"] == client["monitor"])
            expected = [monitor["x"] + (monitor["width"] / monitor["scale"] - 400) / 2,
                        monitor["y"] + (monitor["height"] / monitor["scale"] - 550) / 2]
            assert all(abs(a - b) <= 2 for a, b in zip(client["at"], expected)), client
            current_peer = next(c for c in json.loads(request("j/clients")) if c["pid"] == peer.pid)
            assert (current_peer["at"], current_peer["size"]) == (peer_window["at"], peer_window["size"])
            wait_for(lambda: json.loads(request("j/activewindow")).get("pid") == app.pid)
            print(json.dumps({"cycle": cycle, "window": client}), flush=True)
            time.sleep(0.3)
            if cycle == 1:
                key("p")
                time.sleep(0.2)
                focus_peer = f"/eval hl.dispatch(hl.dsp.focus({{window='pid:{peer.pid}'}}))"
                assert request(focus_peer).strip() == "ok"
                time.sleep(0.3)
                assert clients(), "pinned window must survive focus loss"
                subprocess.run([binary, "--show"], check=True)
                wait_for(lambda: json.loads(request("j/activewindow")).get("pid") == app.pid)
                assert clients()[0]["stableId"] == client["stableId"], "pin activation must reuse window"
                key("p")
                time.sleep(0.2)
                assert request(focus_peer).strip() == "ok"
            else:
                key("-k", "Escape")
                key("-k", "Escape")
            wait_for(lambda: not clients())
        subprocess.run([binary, "--show"], check=True)
        wait_for(lambda: json.loads(request("j/activewindow")).get("pid") == app.pid)
        time.sleep(0.3)
        key("-M", "alt", "-k", "F4", "-m", "alt")
        assert app.wait(timeout=5) == 0
        assert request("/repl _ropy_popup_rule:is_enabled()").strip() == "false"
        print("PASS: hidden startup, --show, reload, shortcut event, geometry, tiling, pin, focus loss, cleanup")
    finally:
        for process in [app, peer]:
            if process is None:
                continue
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
