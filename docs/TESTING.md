- Use `tempfile` to create temporary files.
- Use `rstest` to perform parameterized tests.
- Use appropriate macros to avoid clippy warnings in test functions.
- Test functions should follow the `test_<object>_<scenario>_<expected>` naming pattern.
- Don't write meaningless inevitably correct tests, focus on the execution of the logic.

## Precheck and CI

Run `./scripts/precheck.sh` before committing. It formats locally, checks resources
and script tests, checks unused dependencies when cargo-machete is installed, then
runs Clippy, all-target/all-feature tests, and documentation checks. Clippy covers
the compiler checks, so a separate `cargo check` is unnecessary.

CI uses the same script without modifying files:

- `./scripts/precheck.sh --check --light` verifies Rust formatting, i18n, icons,
  themes, script tests and unused dependencies before compilation.
- `./scripts/precheck.sh --check --rust` runs Clippy, tests and documentation on Linux.
- `./scripts/precheck.sh --check` runs both phases for local CI reproduction.

After the lightweight gate passes, Linux checks and macOS/Windows builds and unit
tests run concurrently. These unit tests do not replace packaged GUI E2E tests.
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
