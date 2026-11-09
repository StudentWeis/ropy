# Release package acceptance

Use this checklist for release candidates and changes to packaging, clipboard, shortcuts, focus, tray or updating. Follow the existing [cross-platform GUI procedure](../.agents/skills/cross-platform-gui-e2e-test/SKILL.md) for isolation and platform automation. Never test against a user's clipboard history or installed application.

## Preconditions

Use a disposable desktop matching the target architecture, with private application data and no host clipboard synchronization. Platform setup, required automation tools and cleanup are owned by the [cross-platform GUI procedure](../.agents/skills/cross-platform-gui-e2e-test/SKILL.md). Do not run install, upgrade or rollback tests over a personal installation.

## Select the artifact

For branch testing, run **Desktop test artifacts** with a full commit SHA; its Linux x86-64 and Windows archives include checksums and the resolved revision. For macOS and Linux ARM64, use the matching Release workflow artifacts. A development build is useful for diagnosis but does not certify a distributed package. Download with `gh run download <run-id>`.

Verify archive checksums against their matching `.sha256` sidecars before launching. The macOS bundle job produces a sidecar for `ropy-<target>-app.tar.xz`; its DMG has no sidecar. Calculate and record the DMG's own SHA-256, and compare it with a publisher-provided DMG digest if available.

Record the workflow run URL, commit SHA, artifact filename, size and SHA-256. Acceptance applies only to those bytes. If publishing rebuilds the package, compare its digest with the tested artifact; a different digest requires another smoke test. Do not claim that testing a branch archive validates a later rebuild.

## Required smoke scenarios

1. Install or unpack in an isolated matching-architecture desktop environment. Launch and verify the process remains alive, initially hidden in the tray.
2. Press Control+Shift+D. Verify both visibility and keyboard focus without manually forcing focus.
3. Copy a unique marker from a native editor and verify it appears in history. Before confirmation, replace only the isolated clipboard with a different sentinel and verify its complete value; clear the destination editor and verify its entire buffer is empty. Reactivate Ropy normally, search for and select the original marker, then confirm. Verify that the complete clipboard value changes from the sentinel to the marker. In copy mode, paste once into the blank editor; in immediate-paste mode, do not send a manual paste. Compare the entire editor buffer with the marker; empty, sentinel, duplicated or extra text fails. A substring match does not pass.
4. Repeat that preparation and assertion with a new marker and sentinel for both copy and paste confirmation modes, and verify focus restoration without corrective focus operations.
5. Open the tray menu, change its language, invoke its actions and quit cleanly. Use a working StatusNotifierItem host for Linux tray rendering checks.
6. Upgrade a disposable installation from the previous stable version. Verify startup, preserved history/settings and managed-install behavior. Exercise failed startup rollback using the updater's existing test procedure; do not substitute unit-test success for actual installation evidence.

Test native Wayland separately using [the VM procedure](wayland-vm.md). An X11 pass on a Wayland host does not cover native Wayland input or activation.

## Acceptance record

Attach this record and evidence to the release preparation PR or release issue. Record each supported target independently; do not turn an unavailable platform into a pass.

| Target | OS / desktop | Artifact SHA-256 | Smoke result | Evidence |
| --- | --- | --- | --- | --- |
| macOS ARM64 | | | Not tested | |
| macOS x86-64 | | | Not tested | |
| Linux x86-64, X11 | | | Not tested | |
| Linux ARM64, X11 | | | Not tested | |
| Windows x86-64 | | | Not tested | |
| Native Wayland (record compositor) | | | Not tested | |

Include source SHA, run URL, tester, timestamps, install/upgrade and rollback results, screenshots of activation/history/search/final paste, and cleanup confirmation. Use **pass**, **application failure**, **infrastructure blocked** or **partial**, with the last verified step. Resolve failures before declaring acceptance; explicitly disclose unsupported or untested combinations.

This is a manual release procedure, not an automated PR status check. Keep the fast PR gate independent of interactive desktops; automate a scenario only after it is repeatable in a disposable environment.
