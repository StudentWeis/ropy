# Testing

Use this guide to choose local checks, reproduce CI and identify tests that need an isolated desktop. Prepare dependencies with [Development](development.md). Commands run from the repository root unless stated otherwise. Unit tests and protocol fixtures do not replace [release package acceptance](release-acceptance.md).

## Test design

- Name test functions `test_<object>_<scenario>_<expected>`.
- Use `rstest` for parameterized cases and `tempfile` for filesystem isolation.
- Place each test at the layer that owns the behavior: core tests for storage, search and settings; GPUI tests for window, board and hotkey behavior.
- Cover the failure and recovery path next to the happy path. A test that cannot fail adds maintenance cost without diagnostics.
- Pass the active locale explicitly; tests and offline fixtures do not read the shared locale global.

A behavior change needs a test that fails without it. Doc tests and a clean Clippy run do not replace that test, and headless tests do not replace the native evidence described below.

## Precheck and CI

Choose the gate by what the change contains. Code, checks, templates, configuration and assets need `./scripts/precheck.sh`; Markdown and agent instructions alone use `./scripts/precheck.sh --check --light`. CI runs both phases on every PR, so a Markdown-only change still has to pass the full gate before merge.

The full gate formats locally, checks resources, Markdown links and script tests, then runs Clippy, all-feature tests whose default targets cover unit, integration and doc tests, and documentation checks across the workspace. Clippy covers the compiler checks, so a separate `cargo check` is unnecessary. Criterion benchmark binaries are type-checked by Clippy but not executed by the gate; `make bench-search` runs them.

CI uses the same script without modifying files:

- `./scripts/precheck.sh --check --light` verifies Rust formatting, i18n, icons, themes, Markdown links and anchors, the vendored GPUI Kit skill snapshots and script tests before compilation.
- `./scripts/precheck.sh --check --rust` runs Clippy, tests, the isolated Linux tray regression and documentation.
- `./scripts/precheck.sh --check` runs both phases for local CI reproduction.

After the lightweight gate passes, core-only Linux tests, Linux desktop checks, and macOS/Windows builds and unit tests run concurrently. These unit tests do not replace packaged GUI E2E tests. The aggregate `Precheck (fmt + clippy + test + i18n/icons/themes)` check passes only when every required job, including the preserved cross-platform check names, passes; [main branch protection](releasing.md#main-branch-protection) records which checks are required.

## Packaged desktop test builds

Use **Actions → Desktop test artifacts → Run workflow** to package an untagged branch before GUI validation. Select the workflow branch and optionally enter a branch, tag or full commit SHA in `ref`; leaving it blank packages the selected workflow revision. The workflow must first exist on the default branch for the manual dispatch entry to be available.

```bash
gh workflow run desktop-test.yml --ref main -f ref=<commit-sha>
gh run list --workflow desktop-test.yml
gh run download <run-id> --dir /tmp/ropy-desktop-test
```

The workflow resolves the source to one commit before starting the native Linux and Windows builds. Each `desktop-test-<target>-<commit>` artifact contains the platform archive, its `.sha256` checksum, `dist-manifest.json` and `source-revision.txt`. Artifacts expire after seven days. Verify the checksum and record the source revision before transferring the package to a clean GUI test environment. These builds use the selected source's cargo-dist version, native runners and system dependencies; they do not create tags or GitHub Releases.

## Promotional captures

See [Promotional screenshots](promo-screenshots.md) for the isolated macOS capture workflow and plain 2×2 output. This checks real rendering; it does not replace clipboard or platform integration tests.

## Architecture boundaries

The lightweight gate includes `scripts/tests/test_architecture.py`. It rejects presentation dependencies in repository/settings modules and GPUI runtime references in clipboard I/O. This source-level check protects the module seams described in [Architecture](architecture.md); behavioral tests remain responsible for persistence, capture acknowledgement, settings recovery and UI interactions.

## Workspace commands

Makefile check, test, Clippy and documentation targets explicitly select the whole workspace; formatting targets use `--all`. Build and run targets keep the desktop application as their default member.

Image capture tests exercise abandoned staging, failed database commits, partial file installation and preservation of existing same-hash payloads. GPUI context tests cover background history refresh, current filters and selection, changed history limits and clearing history with a request pending.

- `cargo test --locked -p ropy-core`: run core unit tests, public API integration tests and doctests without compiling GPUI or native clipboard integration.
- `cargo test --locked --workspace --all-features`: run both packages, including the desktop tests that use the core `test` feature for fault injection. The default target selection covers unit, integration and doc tests without executing the Criterion benchmark binaries.
- `python3 scripts/check/check_core_dependencies.py`: verify the full core dependency graph, including platform-specific and test dependencies.
- `python3 scripts/check/check_markdown.py`: verify relative links and heading anchors in repository Markdown, skipping the vendored GPUI Kit snapshots.
- `python3 scripts/check/check_agent_skills.py`: verify both vendored snapshot digests against `.agents/official-snapshots.json`, and that the adapter records the manifest commit, the declared `gpui-kit` version and the locked `gpui-pre` version.
- `make fmt-check`: check formatting in both packages.

The precheck uses explicit `--workspace` selection because the default member is still the root desktop application. Core CI deliberately installs no GTK/X11 system libraries; its result participates in the required aggregate check.

## Release benchmarks

See [Release benchmark baseline](benchmarks.md) for `make bench`, version comparison, measurement boundaries and release artifact size reports.

## Native X11 popup focus regression

On an [isolated X11 desktop](../.agents/skills/cross-platform-gui-e2e-test/references/linux-local.md), start Ropy with disposable data, open another application to receive focus, and show one unpinned Ropy window. With `WAYLAND_DISPLAY` unset and `DISPLAY` pointing only to that test desktop, run:

```sh
python3 scripts/test_popup_focus.py --output /tmp/ropy-popup-focus
```

The script requires `xdotool` and `xwininfo`; use `--hotkey` if the activation shortcut differs from Ctrl+Shift+D. It leaves the pointer inside the popup, dismisses with Escape and the activation shortcut, and checks that each subsequent shortcut restores both mapping and actual keyboard focus for three cycles. Native focus/map observations are saved even on failure. This protects against stale GPUI activation after X11 `FocusIn(detail=NotifyPointer)` notifications. It is a manual native regression, not part of the headless precheck or a substitute for packaged GUI acceptance. Also run `scripts/test_window_drag.py` when changing native focus handling to cover window-manager keyboard grabs.

## Native Wayland integration

For Linux window, shortcut or clipboard changes, follow the isolated VM procedure in [wayland-vm.md](wayland-vm.md) and the capability boundaries in [wayland.md](wayland.md). Use a native Wayland editor, disable XWayland and unset `DISPLAY`; an X11 clipboard test does not verify native Wayland behavior.

Exercise hidden startup, repeated popup reconstruction, clipboard capture while hidden, search, both confirmation modes and focus restoration. Record the compositor version, source revision, binary digest and activation policy alongside the results. Test pinned-window reactivation separately from a newly mapped window.

Run the portal fixture's accept, deny, pending and startup modes for changes to shortcut registration or activation-token handling. This fixture validates the client protocol contract; it does not replace testing a real desktop portal. Report untested desktops and infrastructure blockers explicitly. Passing CI alone does not certify GNOME, KDE, Niri or Hyprland GUI behavior.

## Linux tray backend

Linux uses the KSNI backend without GTK or AppIndicator. Run its lifecycle regression on a private session bus so it cannot register on your desktop:

```bash
dbus-run-session -- cargo test --locked -p ropy test_tray_without_gtk_initializes_refreshes_and_drops -- --ignored
```

The Rust phase of precheck runs this on Linux, separately from the ordinary unit suite. Install `dbus-run-session` (the `dbus` package on Ubuntu) before running precheck; a missing session-bus tool fails the check rather than skipping tray coverage. It verifies tray creation, icon properties, localized menu labels, all three menu events before and after menu replacement, and teardown without GTK initialization. Real desktop panel rendering, opening application panels from menu actions and panel restart recovery still require GUI validation with a StatusNotifierItem host.

## Reproducible CI gates

The Rust compiler, rustfmt and Clippy share the stable version and components declared in `rust-toolchain.toml`. Install them before local verification:

```sh
rustup show active-toolchain
```

Makefile and precheck Rust validation commands use `--locked`. Dependency changes must include the reviewed `Cargo.lock` update. Upgrade the shared toolchain deliberately in `rust-toolchain.toml`, then run `make fmt` and the full precheck.

Formatting uses stable rustfmt defaults; imports stay sorted within each explicit group. The formatter does not regroup standard-library, external and local imports, and it does not merge crate-level `use` statements.

Main branch protection is server configuration, not something workflow YAML enables itself. See [main branch protection](releasing.md#main-branch-protection) for the required checks and the reviewed API payload. See [Release acceptance](release-acceptance.md) for the manual package smoke gate and its evidence requirements; CI unit tests alone do not certify a release.
