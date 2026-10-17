- Use `tempfile` to create temporary files.
- Use `rstest` to perform parameterized tests.
- Use appropriate macros to avoid clippy warnings in test functions.
- Test functions should follow the `test_<object>_<scenario>_<expected>` naming pattern.
- Don't write meaningless inevitably correct tests, focus on the execution of the logic.

## Precheck and CI

Run `./scripts/precheck.sh` before committing. It formats locally, checks resources
and script tests, then runs Clippy, all-target/all-feature tests, and documentation
checks across the workspace. Clippy covers the compiler checks, so a separate
`cargo check` is unnecessary.

CI uses the same script without modifying files:

- `./scripts/precheck.sh --check --light` verifies Rust formatting, i18n, icons,
  themes and script tests before compilation.
- `./scripts/precheck.sh --check --rust` runs Clippy, tests, the isolated Linux tray regression and documentation.
- `./scripts/precheck.sh --check` runs both phases for local CI reproduction.

After the lightweight gate passes, core-only Linux tests, Linux desktop checks,
and macOS/Windows builds and unit tests run concurrently. These unit tests do not replace packaged GUI E2E tests.
The existing `Precheck (fmt + clippy + test + i18n/icons/themes)` check is now an
aggregate gate: failure, cancellation or skipping of any required job prevents
it from passing. Cross-platform check names are also preserved for branch rules.

Rust caches are restored on all branches and saved only on main. This avoids PR
cache uploads and retains the existing job-based cache keys. A PR that changes
dependencies may need to rebuild them on each run until it merges; compare cache
restore/save durations and end-to-end workflow duration in Actions when evaluating
this tradeoff. Runner queue time can still dominate even with parallel jobs.

## Packaged desktop test builds

Use **Actions → Desktop test artifacts → Run workflow** to package an untagged
branch before GUI validation. Select the workflow branch and optionally enter a
branch, tag or full commit SHA in `ref`; leaving it blank packages the selected
workflow revision. The workflow must first exist on the default branch for the
manual dispatch entry to be available.

```bash
gh workflow run desktop-test.yml --ref main -f ref=<commit-sha>
gh run list --workflow desktop-test.yml
gh run download <run-id> --dir /tmp/ropy-desktop-test
```

The workflow resolves the source to one commit before starting the native Linux
and Windows builds. Each `desktop-test-<target>-<commit>` artifact contains the
platform archive, its `.sha256` checksum, `dist-manifest.json` and
`source-revision.txt`. Artifacts expire after seven days. Verify the checksum and
record the source revision before transferring the package to a clean GUI test
environment. These builds use the selected source's cargo-dist version, native
runners and system dependencies; they do not create tags or GitHub Releases.

## Promotional captures

See [Promotional screenshots](PROMO_SCREENSHOTS.md) for the isolated macOS capture
workflow and plain 2×2 output. This checks real rendering; it does not replace
clipboard or platform integration tests.

## Architecture boundaries

The lightweight gate includes `scripts/tests/test_architecture.py`. It rejects
presentation dependencies in repository/settings modules and GPUI runtime
references in clipboard I/O. This source-level check protects the module seams
described in [Architecture](ARCHITECTURE.md); behavioral tests remain responsible
for persistence, capture acknowledgement, settings recovery and UI interactions.

## Workspace commands

Makefile check, test, Clippy and documentation targets explicitly select the
whole workspace; formatting targets use `--all`. Build and run targets keep
the desktop application as their default member.

Image capture tests exercise abandoned staging, failed database commits,
partial file installation and preservation of existing same-hash payloads.
GPUI context tests cover background history refresh, current filters and
selection, changed history limits and clearing history with a request pending.

- `cargo test --locked -p ropy-core`: run core unit tests, public API integration
  tests and doctests without compiling GPUI or native clipboard integration.
- `cargo test --workspace --all-targets --all-features`: run both packages,
  including the desktop tests that use the core `test` feature for fault injection.
- `python3 scripts/check/check_core_dependencies.py`: verify the full core
  dependency graph, including platform-specific and test dependencies.
- `make fmt-check`: check formatting in both packages.

The precheck uses explicit `--workspace` selection because the default member is
still the root desktop application. Core CI deliberately installs no GTK/X11
system libraries; its result participates in the required aggregate check.

## Release benchmarks

See [Release benchmark baseline](BENCHMARKS.md) for `make bench`, version
comparison, measurement boundaries and release artifact size reports.

## Native Wayland integration

For Linux window, shortcut or clipboard changes, follow the isolated VM procedure
in [WAYLAND_VM.md](WAYLAND_VM.md) and the capability boundaries in
[WAYLAND.md](WAYLAND.md). Use a native Wayland editor, disable XWayland and unset
`DISPLAY`; an X11 clipboard test does not verify native Wayland behavior.

Exercise hidden startup, repeated popup reconstruction, clipboard capture while
hidden, search, both confirmation modes and focus restoration. Record the
compositor version, source revision, binary digest and activation policy alongside
the results. Test pinned-window reactivation separately from a newly mapped window.

Run the portal fixture's accept, deny, pending and startup modes for changes to
shortcut registration or activation-token handling. This fixture validates the
client protocol contract; it does not replace testing a real desktop portal.
Report untested desktops and infrastructure blockers explicitly. Passing CI alone
does not certify GNOME, KDE, Niri or Hyprland GUI behavior.

## Linux tray backend

Linux uses the KSNI backend without GTK or AppIndicator. Run its lifecycle
regression on a private session bus so it cannot register on your desktop:

```bash
dbus-run-session -- cargo test --locked -p ropy test_tray_without_gtk_initializes_refreshes_and_drops -- --ignored
```

The Rust phase of precheck runs this on Linux, separately from the ordinary unit suite.
Install `dbus-run-session` (the `dbus` package on Ubuntu) before running precheck;
a missing session-bus tool fails the check rather than skipping tray coverage. It verifies tray creation,
icon properties, localized menu labels, all three menu events before and after
menu replacement, and teardown without GTK initialization. Real desktop panel
rendering, opening application panels from menu actions and panel restart recovery
still require GUI validation with a StatusNotifierItem host.

## Reproducible CI gates

The Rust compiler is pinned by `rust-toolchain.toml`; the formatter is pinned
separately by `rustfmt-toolchain`. Install both before local verification:

```sh
rustup show active-toolchain
rustup component add clippy
rustup toolchain install "$(cat rustfmt-toolchain)" --profile minimal --component rustfmt
```

Makefile and precheck Rust validation commands use `--locked`. Dependency changes
must include the reviewed `Cargo.lock` update. Upgrade the formatter deliberately
by changing `rustfmt-toolchain` and running `make fmt` and the full precheck.

The main branch requires the aggregate `Precheck (fmt + clippy + test + i18n/icons/themes)`
and Release `plan` checks, with the PR up to date before merge. GitHub branch
protection is server configuration, not something workflow YAML enables itself.
The reviewed API payload is `.github/main-branch-protection.json`. Administrators
can reconcile it with `gh api --method PUT repos/StudentWeis/ropy/branches/main/protection
--input .github/main-branch-protection.json` and read the protection endpoint back
to verify it. It requires a PR without requiring another reviewer, binds checks
to GitHub Actions, applies to admins and disallows force pushes and deletion.
The manual desktop packaging plan has a distinct check name so it cannot satisfy
the Release `plan` requirement.
See [Release acceptance](RELEASE_ACCEPTANCE.md) for the manual package smoke gate
and its evidence requirements; CI unit tests alone do not certify a release.
