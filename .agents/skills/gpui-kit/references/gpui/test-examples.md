# Running and organizing tests

**Contents:** [Repository workflow](#repository-workflow) · [Useful tests](#useful-tests) · [Framework examples](#framework-examples)

## Repository workflow

Follow [Ropy's testing guide](../../../../../docs/TESTING.md) and the
[contribution flow](../../../contribution-flow/SKILL.md). Run the existing
`./scripts/precheck.sh` gate before a commit. Use the relevant existing target
for a focused regression; do not add an unrelated CI workflow or standalone
documentation-test package.

Ropy already enables gpui-kit test-support in its development dependency.
For another application, use the same gpui-kit version/source in its normal
and development dependencies and add test-support as described in
[Testing](test.md#choose-the-test-level). A `gpui/test-support` feature assumes
a dependency named gpui and is not the setup for a gpui-kit-only consumer.

## Useful tests

Group tests around behavior and keep fixtures local to their owning module.
A shared helper should establish real state needed by several tests, not hide
the action and assertions. Assert initial state, drive the production behavior,
then verify the outcome and relevant rejected/empty path.

Use deterministic state/geometry assertions rather than a fixed wall-clock
threshold for performance. `--test-threads=1` serializes test execution; it does
not add property-test iterations. Use property testing only where generated
inputs prove a meaningful invariant, and report the actual test command and
coverage rather than claiming all platform behavior is simulated.

## Framework examples

The pinned upstream [Kit UI integration example](https://github.com/longbridge/gpui-kit/blob/67265a36e260ffe03c5aaebdcd8d654dbf892eab/crates/kit/tests/ui.rs)
shows retained Input state, real input, focus, geometry, and owner-result
assertions. Its supporting files and commands belong to the GPUI Kit checkout;
[Testing](test.md) explains application adaptation. Read
[test scheduling and re-entrancy](test-reference.md) for async and deferred
callbacks. OS clipboard, native window activation, and global-hotkey behavior
still need the repository's applicable platform tests.
