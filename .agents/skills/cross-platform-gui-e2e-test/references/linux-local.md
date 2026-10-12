# Local Linux X11 regression, including Wayland hosts

A Linux host running Wayland can test Ropy's X11 path in an isolated nested
desktop. Use Xephyr with a lightweight window manager such as Openbox for a
visible desktop; Xephyr connects to the host's XWayland `DISPLAY`. Use Xvfb
with a window manager for unattended testing or when the host has no reachable
X display. Neither mode certifies native Wayland behavior or other window
managers/distributions. AgentBay is not a prerequisite for this workflow.

## Prepare the executable and tools

- For a branch regression, build the requested commit, record dirty state,
  architecture, build profile, executable path and SHA-256. For release testing,
  use the matching verified packaged artifact.
- Discover tools before installing anything: Xephyr (or Xvfb), a window manager,
  X11 inspection/input tools such as `xdpyinfo`, `xwininfo`, `xprop`, `xdotool`,
  and a native X11 editor. `xclip` can inspect the isolated clipboard; it does
  not replace the editor interaction scenario.
- Prefer installed tools or distribution packages extracted into a temporary
  tools directory. Scope any `PATH`, `LD_LIBRARY_PATH` and `XDG_DATA_DIRS`
  overrides to test children; verify runtime libraries before launch. Follow
  normal authorization for system installation rather than changing the host
  desktop configuration to enable testing.
- If comparing a baseline, retain distinct executable copies or use separate
  build output directories. Do not accidentally test a baseline binary as the
  current branch; restore any shared build output afterward.

## Create an isolated desktop

1. Create a unique temporary test root and select an unused display number.
   Check both the X socket and lock file; never remove another server's files.
   Start the nested server with TCP listening disabled and record its PID or
   process group immediately. Use server-local authorization; do not alter the
   host's access control. The child display must differ from the host display.
2. Preserve the host environment for launching Xephyr itself. For the window
   manager, Ropy, editor and automation children, set `DISPLAY` to the nested
   server and remove `WAYLAND_DISPLAY` and `WAYLAND_SOCKET`. Use `GDK_BACKEND=x11`
   for GTK helpers and `XDG_SESSION_TYPE=x11`; clear inherited compositor or
   scaling overrides unless they are part of the test.
3. Give the children separate `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
   `XDG_CACHE_HOME`, `TMPDIR` and a mode-0700 `XDG_RUNTIME_DIR` under the test root.
   Run a private session D-Bus, for example through `dbus-run-session`. Trace the
   application's settings, database, logs and autostart paths to confirm they
   resolve inside the test root. Do not reuse the user's existing Ropy instance.
4. Start the window manager on the nested display. Verify connectivity with
   `xdpyinfo` and verify that the window manager manages an editor window.
   Confirm the editor is using X11, not a native Wayland connection inherited
   from the host. A tray host is optional unless testing tray-specific behavior.
5. Send input and capture screenshots only on the nested display. Keep its
   clipboard separate; do not enable host clipboard synchronization. Record
   every child process and put teardown in a `finally` block or shell trap so
   failures cannot leave the nested desktop running.

Apply environment changes to child processes, not the running desktop or
system-wide shell configuration. Keep the nested server and window manager
healthy when varying Ropy's launch environment.

## Exercise the X11 workflow

Run the skill's shared GUI scenario, then the relevant branch-specific checks:

- For runtime-directory compatibility, launch Ropy with `XDG_RUNTIME_DIR` set,
  absent, and empty. Apply each variation to both the first instance and its
  activation clients. Verify hidden startup, `--show`, forwarding without a
  second resident process, and restart after killing only the test instance.
- Activate from the focused editor using `Control+Shift+D` and `--show`.
  Inspect map state **and** X keyboard focus; send Escape through normal input,
  assert hiding, then reopen. Verify focus returns to the intended editor.
  Also test reopening after dismissal; record which application owns focus.
- Click a header button using native pointer input after inspecting a fresh
  screenshot, and verify its panel opens. A component/headless test alone does
  not establish native pointer delivery.
- Confirm the unique copied marker appears in history and filtered results.
  In copy mode, verify clipboard write-back and paste in the editor. When
  immediate paste is in scope, enable it only in the isolated settings, clear
  the destination after copying, and assert the editor receives the marker
  exactly once without a manual paste or corrective focus operation.

If a normal step fails, capture focus, mapping, window properties and logs
before any diagnostic workaround. `xdotool windowfocus`, direct-to-window key
injection or manually raising a window may help isolate later behavior; label
those results as diagnostic and keep the original assertion failed. Missing
tray hosting or unavailable rendering infrastructure is not by itself a Ropy
regression. Compare the same scenario on the branch baseline before claiming
that a discovered failure is newly introduced.

## Evidence and cleanup

Record the host session separately from the tested session, for example
`Wayland host / Xephyr X11 / Openbox / local debug build`. Preserve per-assertion
results, exact marker assertions, screenshots, application logs from the
isolated config directory, and baseline results when used. A local pass applies
only to that combination; it is not packaged cross-platform certification.

Stop only the recorded test application, editor, window manager, nested server
and private bus. Verify those processes exited and the nested display socket
is gone. Keep useful evidence at a reported location; remove temporary package
archives, extracted tools and extra build copies. Leave the host desktop,
clipboard, real Ropy data and repository sources unchanged by the test run.
