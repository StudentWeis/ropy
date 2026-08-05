---
name: cross-platform-gui-e2e-test
description: Run packaged Ropy desktop GUI end-to-end, smoke, and compatibility tests across macOS, Windows, and Linux. Use when validating a Ropy release or CI artifact, reproducing platform-specific clipboard, global-hotkey, tray, focus, X11, or window behavior, or collecting cross-platform GUI evidence. Run macOS locally; run Windows and Linux in isolated AgentBay virtual desktops.
---

# Ropy Cross-Platform GUI E2E Test

Validate the packaged desktop application on every supported operating system. Complement GPUI in-process tests with real process, clipboard, global-hotkey, tray, focus, and window interactions.

## Enforce the Platform Contract

| Target | Execution environment | Release artifact |
| --- | --- | --- |
| macOS Apple Silicon | Local matching Mac | `ropy-aarch64-apple-darwin.dmg` |
| macOS Intel | Local matching Mac | `ropy-x86_64-apple-darwin.dmg` |
| Windows x86-64 | AgentBay Windows virtual desktop | `ropy-x86_64-pc-windows-msvc.zip` |
| Linux x86-64 | AgentBay Linux X11 virtual desktop | `ropy-x86_64-unknown-linux-gnu.tar.xz` |
| Linux ARM64 | Matching AgentBay custom Linux X11 image | `ropy-aarch64-unknown-linux-gnu.tar.xz` |

- Inspect the host or guest architecture before selecting an artifact. Never use emulation silently.
- Run macOS on the local Mac. Do not spend AgentBay resources for macOS unless the user explicitly changes this policy.
- Run every Windows and Linux GUI test in a fresh virtual desktop. Do not substitute a local container, headless shell, WSL, or compile-only CI job.
- Require X11 connectivity for Linux. Ropy's Linux GUI integration is X11-specific; a `DISPLAY` variable without a reachable X server is insufficient.
- Report a cross-platform pass only when every requested platform row passes. Preserve separate results for each OS and architecture.

Read only the reference for the platform being exercised:

- [macOS local run](references/macos.md)
- [Windows AgentBay run](references/windows.md)
- [Linux AgentBay run](references/linux.md)

## Apply Shared Guardrails

- Resolve the release tag or commit requested by the user. Otherwise use the latest published release.
- Download artifacts to a temporary controller directory, inspect their contents, calculate SHA-256, and compare any publisher-provided digest before execution.
- Keep AgentBay credentials in an environment variable, secret store, or user-level MCP configuration. Never commit, print, or include a credential-bearing URL in output.
- Confirm paid-session authorization unless the current request already authorizes the virtual test. Do not enable postpaid billing, buy credits, or raise limits without separate authorization.
- Before creating a virtual session, inspect active sessions and billing controls. Prefer concurrency `1`, a maximum runtime of no more than `30` minutes, and an idle timeout of at least `10` minutes.
- Create only one virtual session at a time. Record its identifier immediately and release it in a `finally` path. Explicitly terminate it; do not leave it running or hibernating.
- Use a unique marker such as `ropy-e2e-<platform>-<uuid>` so stale clipboard contents cannot create a false pass.
- Never record unrelated local clipboard contents, credentials, signed URLs, or user data in screenshots and logs.

## Prefer Stable Virtual Control

For Windows and Linux, prefer these AgentBay paths in order:

1. Use a configured MCP endpoint with command, filesystem, screenshot, keyboard, and window capabilities.
2. Use the official AgentBay SDK with `AGENTBAY_API_KEY` supplied through the environment.
3. Use an AgentBay console debug session through the in-app browser only as a manual fallback.

Discover current tool names rather than assuming one SDK version. Prefer filesystem `write_large_file` or an AgentBay Context for artifact transfer. Avoid downloading release assets inside the guest: slow downloads may trigger idle-session reclamation.

If no supported control path is available, stop before creating a paid session and report the missing capability. Never place the secret endpoint in the repository as a workaround.

## Run the Shared GUI Scenario

Use platform-native UI automation, screenshots, and window inspection at every assertion boundary:

1. Launch the verified packaged executable and assert that its process remains alive after three seconds.
2. Expect no initial visible window: Ropy intentionally starts hidden in the system tray.
3. Send `Control+Shift+D`, the default activation hotkey on all supported platforms, and assert that the Ropy window becomes visible.
4. Open a platform-native plain-text editor, enter the unique marker, select it, and copy it.
5. Wait briefly for clipboard ingestion, reactivate Ropy, and assert that the exact marker appears in history.
6. Press `/`, search for a unique substring, and assert that the matching record remains visible.
7. Select the record and press `Enter`. The default confirm mode copies the record back to the clipboard.
8. Return to the editor, paste, and assert that the pasted text exactly equals the marker.

Prefer semantic window, text, and accessibility queries. Use coordinates only after taking a fresh screenshot. Retry a flaky clipboard ingestion once with a new marker; collect state before retrying.

## Add Focused Checks Deliberately

- Test favorite, delete, clear, plain-text paste, grid navigation, pinning, settings, theme, language, opacity, autostart, or update behavior only when relevant to the requested change.
- Avoid destructive history operations on local macOS by default because they can touch the user's real Ropy data.
- Keep the common smoke scenario identical across platforms; isolate platform-specific assertions in the relevant run.
- Verify Linux tray behavior only when the image has a working desktop panel and session bus. Do not classify a missing tray host as a Ropy regression.

## Record a Platform Matrix

For every requested target, record:

- OS version, architecture, execution environment, and image identifier when virtual;
- release tag or commit SHA, artifact name, size, and SHA-256;
- start/end timestamps and elapsed time;
- checksum, extraction, launch, process, window, and assertion results;
- screenshots of the visible Ropy window, captured marker, filtered result, and final editor paste;
- cleanup confirmation and remaining active virtual-session count.

Classify each row independently:

- **pass**: the packaged executable launches and all requested GUI assertions pass;
- **application failure**: the verified artifact runs in a healthy environment but exits, cannot activate, or violates an assertion;
- **infrastructure blocked**: architecture, upload, MCP/SDK access, X11, desktop image, session expiry, or provisioning prevents reaching the assertion;
- **partial**: artifact transfer or launch is proven but the GUI scenario does not finish.

State the last verified boundary. Never describe an infrastructure interruption as a Ropy failure.

## Clean Up Reliably

- Collect platform logs before teardown on failure; use the location in the platform reference.
- Terminate only the Ropy process started by this run and close helper applications when practical.
- Remove temporary local mounts and files.
- Always release AgentBay sessions after Windows or Linux runs, including after exceptions and timeouts.
- Recheck the AgentBay session list and report any cleanup failure immediately.

Before changing the virtual workflow, consult current official AgentBay Computer Use, desktop UI automation, filesystem, command, application, and window-management documentation.
