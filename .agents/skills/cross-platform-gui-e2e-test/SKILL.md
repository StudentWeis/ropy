---
name: cross-platform-gui-e2e-test
description: Run Ropy desktop GUI end-to-end, smoke, and compatibility tests across macOS, Windows, and Linux. Use when validating a Ropy release or CI artifact, reproducing platform-specific clipboard, global-hotkey, tray, focus, X11, or window behavior, or collecting cross-platform GUI evidence. Run macOS locally, Windows in AgentBay, and Linux in AgentBay or a local isolated X11 desktop, including on Wayland hosts.
---

# Ropy Cross-Platform GUI E2E Test

Validate release artifacts across supported operating systems or run focused regressions against a specified branch build. Complement GPUI in-process tests with real process, clipboard, global-hotkey, tray, focus, and window interactions.

## Choose the Test Environment

| Target | Execution environment | Release artifact |
| --- | --- | --- |
| macOS Apple Silicon | Local matching Mac | `ropy-aarch64-apple-darwin.dmg` |
| macOS Intel | Local matching Mac | `ropy-x86_64-apple-darwin.dmg` |
| Windows x86-64 | AgentBay Windows virtual desktop | `ropy-x86_64-pc-windows-msvc.zip` |
| Linux x86-64 | AgentBay Linux X11 desktop or local isolated X11 desktop | `ropy-x86_64-unknown-linux-gnu.tar.xz` |
| Linux ARM64 | Matching AgentBay custom image or local ARM64 X11 desktop | `ropy-aarch64-unknown-linux-gnu.tar.xz` |

- Inspect the host or guest architecture before selecting an artifact. Never use emulation silently.
- Run macOS on the local Mac in an isolated test account and graphical session, as required by the macOS reference. Isolate before launching any artifact. Do not spend AgentBay resources for macOS unless the user explicitly changes this policy.
- Run Windows in a fresh AgentBay desktop. For Linux, honor the requested environment: local X11 regression is supported on both X11 and Wayland hosts. Prefer a fresh AgentBay desktop for packaged distribution compatibility; a local nested desktop proves only its tested host/server/window-manager combination.
- A Wayland host does not prevent X11 testing: Xephyr plus a window manager provides an isolated X11 session, displayed through the host XWayland connection. Xvfb plus a window manager is an alternative for unattended X11 tests. A shell, bare display variable, or compile-only job is not GUI evidence.
- Require a reachable X server and window manager for Linux X11 tests. Test the nested X11 application path separately from native Wayland clipboard/input or host-compositor integration.
- Report a cross-platform pass only when every requested platform row passes. Preserve separate results for each OS and architecture.

Read only the reference for the platform being exercised:

- [macOS local run](references/macos.md)
- [Windows AgentBay run](references/windows.md)
- [Linux local X11 run, including Wayland hosts](references/linux-local.md)
- [Linux AgentBay run](references/linux.md)

## Apply Shared Guardrails

- Resolve the requested release or commit. For branch regressions, build that branch and record the commit and dirty state; for release validation without a version, use the latest published release. Label development builds separately from packaged artifacts.
- Download release artifacts to a temporary controller directory, inspect their contents, calculate SHA-256, and compare any publisher-provided digest before execution. For local builds, record the executable digest after building.
- Keep AgentBay credentials in an environment variable, secret store, or user-level MCP configuration. Never commit, print, or include a credential-bearing URL in output.
- Confirm paid-session authorization unless the current request already authorizes the virtual test. Do not enable postpaid billing, buy credits, or raise limits without separate authorization.
- Before creating an AgentBay session, inspect active sessions and billing controls. Prefer concurrency `1`, a maximum runtime of no more than `30` minutes, and an idle timeout of at least `10` minutes.
- Create only one AgentBay session at a time. Record its identifier immediately and release it in a `finally` path. Explicitly terminate it; do not leave it running or hibernating.
- Use a unique marker such as `ropy-e2e-<platform>-<uuid>` so stale clipboard contents cannot create a false pass.
- Never record unrelated local clipboard contents, credentials, signed URLs, or user data in screenshots and logs.

## Prefer Stable Virtual Control

When AgentBay is the selected environment, prefer these control paths in order:

1. Use a configured MCP endpoint with command, filesystem, screenshot, keyboard, and window capabilities.
2. Use the official AgentBay SDK with `AGENTBAY_API_KEY` supplied through the environment.
3. Use an AgentBay console debug session through the in-app browser only as a manual fallback.

Discover current tool names rather than assuming one SDK version. Prefer filesystem `write_large_file` or an AgentBay Context for artifact transfer. Avoid downloading release assets inside the guest: slow downloads may trigger idle-session reclamation.

If no supported control path is available, stop before creating a paid session and report the missing capability. Never place the secret endpoint in the repository as a workaround.

## Run the Shared GUI Scenario

Use platform-native UI automation, screenshots, and window inspection at every assertion boundary:

1. Launch the verified executable and assert that its process remains alive after three seconds.
2. Expect no initial visible window: Ropy intentionally starts hidden in the system tray.
3. Send `Control+Shift+D`, the default activation hotkey on all supported platforms, and assert that the Ropy window becomes visible and receives keyboard focus.
4. Open a platform-native plain-text editor, enter the unique marker, select it, and copy it.
5. Wait briefly for clipboard ingestion, reactivate Ropy, and assert that the exact marker appears in history.
6. Press `/`, search for a unique substring, and assert that the matching record remains visible.
7. Select the record and press `Enter`. The default confirm mode copies the record back to the clipboard.
8. Return to the editor, paste, and assert that the pasted text exactly equals the marker.

A mapped window is not proof of focus. If activation fails, record it before forcing focus or directing keys to a window; such workarounds can diagnose later steps but cannot turn the normal workflow into a pass.

Prefer semantic window, text, and accessibility queries. Use coordinates only after taking a fresh screenshot. Retry a flaky clipboard ingestion once with a new marker; collect state before retrying.

## Add Focused Checks Deliberately

- Test favorite, delete, clear, plain-text paste, grid navigation, pinning, settings, theme, language, opacity, autostart, or update behavior only when relevant to the requested change.
- Run local macOS history operations only against the isolated test account's disposable data. Normal startup can modify history too; skipping delete/clear buttons is not isolation.
- Keep the common smoke scenario identical across platforms; isolate platform-specific assertions in the relevant run.
- Verify Linux tray behavior only when the image has a working desktop panel and session bus. Do not classify a missing tray host as a Ropy regression.

## Record a Platform Matrix

For every requested target, record:

- OS version, architecture, host session type, tested display server/window manager, execution environment, and image identifier when virtual;
- release tag or commit SHA, artifact name, size, and SHA-256;
- start/end timestamps and elapsed time;
- checksum, extraction, launch, process, window, and assertion results;
- screenshots of the visible Ropy window, captured marker, filtered result, and final editor paste;
- cleanup confirmation and remaining active virtual-session count.

Classify each row independently:

- **pass**: the selected executable launches and all requested GUI assertions pass in the recorded environment; identify whether it is a development or packaged build;
- **application failure**: the verified artifact runs in a healthy environment but exits, cannot activate, or violates an assertion;
- **infrastructure blocked**: architecture, upload, MCP/SDK access, X11, desktop image, session expiry, or provisioning prevents reaching the assertion;
- **partial**: artifact transfer or launch is proven but the GUI scenario does not finish.

State the last verified boundary. Never describe an infrastructure interruption as a Ropy failure.

## Clean Up Reliably

- Collect platform logs before teardown on failure; use the location in the platform reference.
- Terminate only the Ropy process started by this run and close helper applications when practical.
- Remove temporary mounts and disposable files after retaining the evidence needed for the report.
- For local X11 runs, stop the recorded application/helper/server process groups and private session bus; verify the nested display is gone. Retain requested evidence, remove temporary tools/build copies, and preserve the host desktop and clipboard.
- For AgentBay runs, always release sessions, including after exceptions and timeouts. Recheck the session list and report any cleanup failure immediately.

Before changing AgentBay-specific control steps, consult current official AgentBay Computer Use, desktop UI automation, filesystem, command, application, and window-management documentation.
