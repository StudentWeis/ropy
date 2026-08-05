# macOS Local Run

Use the local Mac for macOS GUI validation. Inspect `uname -m` first and select the matching DMG; do not run an Intel artifact on Apple Silicon through Rosetta without explicitly labeling that separate compatibility run.

## Protect Local State

- Check for an existing Ropy process. Do not terminate or replace a user-owned running instance without explicit permission.
- Warn that the smoke test temporarily changes the system clipboard and can add the unique marker to the user's Ropy history.
- Never run clear-history or bulk-delete checks locally by default. Remove only the unique test marker when that can be done unambiguously.
- Do not inspect or report pre-existing clipboard history.

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

- Collect JSONL logs from `~/Library/Application Support/ropy/logs` on failure.
- Quit only the Ropy instance launched for this test.
- Eject the mounted DMG with `hdiutil detach` and remove the temporary download.
- Restore the prior plain-text clipboard value when it was captured without exposing it; otherwise state that clipboard restoration was not lossless.
