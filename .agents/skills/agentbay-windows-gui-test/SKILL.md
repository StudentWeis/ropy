---
name: agentbay-windows-gui-test
description: Deploy and exercise Ropy Windows release builds in Alibaba Cloud AgentBay. Use when asked to run Ropy Windows GUI end-to-end, smoke, or compatibility tests; validate a packaged ropy.exe; reproduce Windows-only clipboard, global-hotkey, or tray behavior; or collect evidence from an AgentBay Windows sandbox.
---

# AgentBay Windows GUI Test

Run Ropy as a real packaged desktop application in an isolated AgentBay Windows session. Treat this as system-level coverage that complements, rather than replaces, GPUI's in-process tests.

## Guardrails

- Use an AgentBay Windows Computer Use image. Do not substitute a browser-only or Linux image.
- Keep the API key in an environment variable, secret store, or user-level MCP configuration. Never commit it, paste it into source files, print it, or include a credential-bearing MCP URL in output.
- Confirm paid-session authorization unless the current user request already authorizes running the test. Do not enable postpaid billing, buy credits, or raise account limits without separate authorization.
- Before creating a session, inspect active sessions and billing controls. Prefer concurrency `1`, a maximum run of no more than `30` minutes, and an idle timeout of at least `10` minutes for artifact setup.
- Create only one session. Record its identifier immediately and release it in a `finally` path. Explicitly terminate it; do not leave it running or hibernating.
- Use a fresh sandbox for release validation. Do not rely on state from an earlier run.

## Choose the Control Path

Prefer these paths in order:

1. Use a configured AgentBay MCP endpoint with Windows command, filesystem, screenshot, keyboard, and window tools.
2. Use the official AgentBay SDK with the API key supplied through the environment.
3. Use the AgentBay console debug session through the in-app browser only as a manual fallback.

Discover the available tool names rather than assuming one SDK version. For large artifact transfer, prefer the filesystem `write_large_file` capability or an AgentBay Context. Avoid downloading GitHub assets from inside the guest: guest networking can be slow, and browser downloads may not count as activity for idle-session retention.

If MCP is not configured or unreachable, stop after documenting the missing capability. Do not put the secret URL in the repository to work around it.

## Run the Workflow

### 1. Resolve and inspect the artifact

- Use the release or commit requested by the user. Otherwise select the latest published GitHub release.
- Require the Windows target `x86_64-pc-windows-msvc` and an archive containing `ropy.exe`.
- Download the artifact to a temporary local directory and inspect the archive before upload.
- Calculate the local SHA-256. If the publisher exposes a digest, compare it now. Preserve the calculated digest for the guest-side check.
- Record the release tag or commit SHA, asset URL/name, archive size, and digest.

Do not build Windows binaries on macOS merely because Cargo is installed. Prefer the repository's Windows CI artifact or release package unless a Windows-compatible cross-build toolchain has already been established.

### 2. Create and prepare the Windows session

- Recheck that no stale AgentBay session is active.
- Create one Windows session and record the session and image identifiers.
- Create `C:\ropy-e2e` in the guest.
- Upload the archive as `C:\ropy-e2e\ropy.zip` with the filesystem API.
- Keep the session active during a long transfer with a harmless status request when the platform requires it.

Use shell or command APIs for setup. Do not type long PowerShell commands through remote-desktop keyboard automation because punctuation and keyboard layouts can corrupt them.

### 3. Verify, extract, and launch

Run an equivalent non-interactive PowerShell sequence in the guest:

```powershell
$ErrorActionPreference = 'Stop'
$root = 'C:\ropy-e2e'
$zip = Join-Path $root 'ropy.zip'
$expected = '<local-sha256-lowercase>'
$actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "SHA-256 mismatch: $actual" }
$app = Join-Path $root 'app'
Expand-Archive -Path $zip -DestinationPath $app -Force
$exe = Join-Path $app 'ropy.exe'
if (-not (Test-Path $exe)) { throw "Missing $exe" }
$process = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 3
if ($process.HasExited) { throw "ropy.exe exited with $($process.ExitCode)" }
$process.Id
```

Do not interpret the lack of an initial visible window as failure. Ropy intentionally starts hidden in the system tray. On Windows, its default activation hotkey is `Ctrl+Shift+D`.

### 4. Run the core GUI smoke test

Use screenshots and window inspection at every assertion boundary:

1. Assert that `ropy.exe` remains alive after startup.
2. Send `Ctrl+Shift+D`; assert that the Ropy window becomes visible and capture a screenshot.
3. Open Notepad, enter a unique marker such as `ropy-e2e-<uuid>`, select it, and copy it.
4. Wait for clipboard ingestion, activate Ropy again, and assert that the exact marker appears in history.
5. Press `/`, search for a unique substring of the marker, and assert that the matching record remains visible.
6. Select the record and press `Enter`. The default confirm mode copies the record back to the clipboard.
7. Return to Notepad, paste, and assert that the pasted text exactly equals the marker.

Use a unique value on every run so stale clipboard contents cannot produce a false pass. Prefer semantic window, text, and accessibility queries when available; use coordinates only after capturing a fresh screenshot.

### 5. Add focused checks only when requested

- Press `F` to favorite the selected record and verify its visual state.
- Press `Delete` or `D` and verify deletion confirmation and removal.
- Press `Shift+Enter` on a rich-text record to verify plain-text confirmation.
- Change to `paste_immediately` in Settings and verify that confirmation returns focus and inserts text into the originating application.
- Exercise grid navigation with `H` and `L`, or test theme, language, opacity, pinning, and autostart.

Keep the default smoke test short. Add behavior-specific checks for the change under test rather than turning every run into a full regression suite.

### 6. Collect evidence and clean up

Record:

- release tag or commit SHA, artifact digest, Windows image, and session identifier;
- start/end timestamps and elapsed time;
- command results for checksum, extraction, process launch, and process status;
- screenshots for the visible Ropy window, captured marker, filtered result, and final Notepad paste;
- each assertion as pass, fail, or infrastructure-blocked.

On application failure, collect Ropy JSONL logs from the user's Windows configuration directory under `ropy\logs` before releasing the session. Redact credentials, signed URLs, clipboard data unrelated to the unique test marker, and other user information.

Always terminate `ropy.exe`, close helper applications when practical, and release the AgentBay session in cleanup, including after exceptions or timeouts. Verify from the session list that no session remains active.

## Classify the Result

- Report **pass** only after the executable launches and all requested GUI assertions succeed.
- Report **application failure** when the verified artifact launches in a healthy session but Ropy exits, cannot be activated, or violates a GUI assertion. Include logs and screenshots.
- Report **infrastructure blocked** when upload, AgentBay networking, MCP availability, session expiry, or image provisioning prevents reaching the application assertion. Do not describe this as a Ropy failure.
- Report **partial** when artifact transfer or process launch was proven but the GUI assertions did not complete. State the last verified boundary precisely.

## Troubleshoot Known Failure Modes

- If a console playground rejects custom code before creating a session, use MCP, SDK, or an image-management debug session; the preset playground may restrict arbitrary code.
- If PowerShell 5 cannot download from GitHub, upload the artifact from the controller instead of weakening TLS settings.
- If the archive downloads but the session disappears before extraction, increase idle timeout to at least `10` minutes and use direct file upload.
- If typed commands replace `:` or other punctuation, stop using remote keyboard input for setup and use the command API.
- If the process is alive but no window is visible, send `Ctrl+Shift+D` or use the tray Show action before diagnosing a startup defect.
- If a clipboard assertion is flaky, refocus Notepad, repeat the copy with a new marker, wait briefly, and capture the clipboard/window state before retrying once.

For current platform capabilities, consult the official AgentBay Windows Computer Use, FileSystem, command, UI automation, and window-management documentation before changing the workflow.
