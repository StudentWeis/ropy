# macOS Local Run

Use the local Mac for macOS GUI validation. Inspect `uname -m` first and select the matching DMG; do not run an Intel artifact on Apple Silicon through Rosetta without explicitly labeling that separate compatibility run.

## Isolate the test session before launch

Use a dedicated macOS test account and its own logged-in graphical session. Use
an existing disposable account, or obtain authorization before creating one.
A temporary download directory or mounted DMG does not isolate application data.
Do not launch a test artifact in the everyday account, even when Ropy is closed:
startup can migrate and clear an incompatible history schema, trim history, and
synchronize Login Items before any smoke-test interaction.

Before launching:

- Confirm the graphical session and automation belong to the test account.
  A shell switched with `sudo -u`, a changed `HOME`, or XDG overrides alone do
  not isolate macOS GUI services, clipboard, Keychain or Login Items.
- Resolve the test account's configuration, database, image, rich-text and log
  paths from the tested revision. Current macOS builds use
  `~/Library/Application Support/ropy`. Confirm these paths belong to the test
  account and contain no valuable data. Use a fresh test account when validating
  default settings; do not silently delete a reused account's data.
- Inspect processes in that session. Do not terminate a user-owned process or
  reuse an already-running Ropy instance as the test artifact.
- Keep clipboard markers, screenshots, logs and autostart checks inside that
  session. Do not inspect the everyday account's history or clipboard.

If an isolated graphical session is unavailable, report the GUI run as
infrastructure blocked. In-process tests and promotional renders remain useful
but do not substitute for native clipboard/hotkey E2E evidence.

## Verify and launch

Download the matching `.dmg` to a temporary directory and verify it against the release digest:

```bash
shasum -a 256 /path/to/ropy-apple-darwin.dmg
hdiutil attach -nobrowse -readonly /path/to/ropy-apple-darwin.dmg
```

Locate `Ropy.app` on the mounted volume and launch that exact bundle with `open`. Record the mount point, bundle path, bundle version, and process identifier. Do not validate an already-installed copy by accident.

Use local Computer Use or another approved macOS UI-control capability for screenshots, keyboard input, focus, and the shared GUI scenario. Ropy starts hidden; send `Control+Shift+D` to reveal it.

If macOS blocks the downloaded application through quarantine or Gatekeeper, capture the dialog and report an application packaging/signing result. Do not remove quarantine attributes or bypass platform security unless the user explicitly asks for that diagnostic.

Use a plain-text editor such as TextEdit in plain-text mode for the marker copy/paste assertion.

## Collect and clean up

- Collect JSONL logs from the verified test account data directory on failure.
- Quit only the Ropy instance launched for this test.
- Eject the mounted DMG with `hdiutil detach` and remove the temporary download.
- Confirm all test processes and mounts are gone; leave the everyday account's clipboard, history and Login Items untouched. Retain evidence separately before disposing of the test account through the authorized account-management workflow.
