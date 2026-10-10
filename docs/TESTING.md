- Use `tempfile` to create temporary files.
- Use `rstest` to perform parameterized tests.
- Use appropriate macros to avoid clippy warnings in test functions.
- Test functions should follow the `test_<object>_<scenario>_<expected>` naming pattern.
- Don't write meaningless inevitably correct tests, focus on the execution of the logic.

## Precheck and CI

Run `./scripts/precheck.sh` before committing. It formats locally, checks resources
and script tests, checks
unused dependencies when cargo-machete is installed, then
runs Clippy, all-target/all-feature tests, and documentation checks across the workspace. Clippy covers
the compiler checks, so a separate `cargo check` is unnecessary.

CI uses the same script without modifying files:

- `./scripts/precheck.sh --check --light` verifies Rust formatting, i18n, icons,
  themes, script tests and unused dependencies before compilation.
- `./scripts/precheck.sh --check --rust` runs Clippy, tests and documentation on Linux.
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
- `cargo +nightly fmt --all --check`: check formatting in both packages.

The precheck uses explicit `--workspace` selection because the default member is
still the root desktop application. Core CI deliberately installs no GTK/X11
system libraries; its result participates in the required aggregate check.

## Release benchmarks

See [Release benchmark baseline](BENCHMARKS.md) for `make bench`, version
comparison, measurement boundaries and release artifact size reports.
