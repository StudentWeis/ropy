# Linux AgentBay Run

Use a fresh AgentBay Linux Computer Use virtual desktop. Ropy supports Linux through X11, so require a real desktop session; a successful shell command or headless build is not GUI evidence.

## Match the image

- Inspect `uname -m` and select the matching GNU/Linux artifact. Use `x86_64-unknown-linux-gnu` for an x86-64 `linux_latest` image.
- Use an ARM64 custom image for `aarch64-unknown-linux-gnu`; never execute it through implicit emulation.
- Require a non-empty `DISPLAY` and verify the connection with `xdpyinfo` or an equivalent X11 query.
- Accept XWayland only when the X11 connection is reachable and Ropy's window, hotkey, and clipboard behavior work. Do not claim native Wayland support.
- Check that a desktop window manager is running. Require a desktop panel and session bus only for tray-specific assertions.

## Transfer and launch

Upload the verified archive to `/tmp/ropy-e2e/ropy.tar.xz` with the AgentBay filesystem API. Run an equivalent shell sequence:

```bash
set -eu
root=/tmp/ropy-e2e
archive="$root/ropy.tar.xz"
expected='<local-sha256-lowercase>'
actual="$(sha256sum "$archive" | awk '{print $1}')"
test "$actual" = "$expected"
mkdir -p "$root/app"
tar -xJf "$archive" -C "$root/app"
exe="$(find "$root/app" -type f -name ropy -perm -u+x -print -quit)"
test -n "$exe"
test -n "${DISPLAY:-}"
xdpyinfo >/dev/null
ldd "$exe" | tee "$root/ldd.txt"
if grep -q 'not found' "$root/ldd.txt"; then exit 64; fi
nohup "$exe" >"$root/ropy.stdout.log" 2>"$root/ropy.stderr.log" &
pid=$!
sleep 3
kill -0 "$pid"
printf '%s\n' "$pid"
```

If `ldd` reports missing libraries, classify the stock image as infrastructure-blocked or use an approved custom image containing the runtime equivalents of the repository's GTK3, X11, `libxdo`, and `libxkbcommon-x11` build dependencies. Do not install arbitrary packages silently into a supposedly reproducible compatibility image.

Discover an installed graphical text editor and use it for the shared marker scenario. Send `Control+Shift+D` to reveal Ropy. Use AgentBay screenshot, keyboard, application, and window tools rather than `xdotool` for the assertions unless a lower-level diagnostic is specifically required.

## Troubleshoot and clean up

- If `DISPLAY` is unset or `xdpyinfo` fails, the image is not ready for Ropy GUI testing.
- If the process lives but activation fails, collect X11 session details and Ropy logs before classifying the result.
- If the tray is absent while hotkey activation works, check the desktop panel and `DBUS_SESSION_BUS_ADDRESS`; report missing host infrastructure separately.
- Collect JSONL logs from `~/.config/ropy/logs` plus `$root/ropy.stdout.log` and `$root/ropy.stderr.log` on failure.
- Stop the test process by its recorded PID, then release the AgentBay session and verify that it is gone.

Current official references: [Computer Use](https://help.aliyun.com/en/agentbay/computeruse), [desktop UI automation](https://help.aliyun.com/en/agentbay/developer-reference/ui-automation), and [application management](https://help.aliyun.com/en/agentbay/developer-reference/application-management).
