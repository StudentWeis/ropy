# Temporary block 0.1.6 compatibility patch

This directory preserves the MIT-licensed `block` 0.1.6 crate published by
Steven Sheldon at <https://crates.io/crates/block/0.1.6>. The upstream source is
<https://github.com/SSheldon/rust-block>. Checked on 2026-10-13: the published
version and upstream master still declare `_NSConcreteStackBlock` using an
empty enum.

The local source changes are limited to the foreign symbol boundary:

- Use an inhabited, private, `repr(C)` opaque struct instead of an empty enum.
- Take the symbol's address with `ptr::addr_of!` without forming a reference.
- Explicitly spell the existing C ABI on foreign declarations and callbacks.

The public API, block layout, copy/dispose helpers and reference counting remain
unchanged. The [Clang Blocks ABI](https://clang.llvm.org/docs/Block-ABI-Apple.html)
requires the symbol's address in `isa`, not its contents. The
[Rustonomicon FFI guide](https://doc.rust-lang.org/nomicon/ffi.html#representing-opaque-structs)
explains why empty enums must not be used for this boundary.

The manifest removes the unpublished relative `test_utils` path and uses the
same published `objc_test_utils` version requirement. Upstream source tests
remain available separately; this crate is excluded from Ropy's workspace so
the legacy code does not inherit application lints or formatting.

Ropy's lightweight gate compiles this source directly with
`-D uninhabited_static`, bypassing Cargo's dependency lint cap. Its macOS test
target `tests/block_compat.rs` exercises typed callback invocation, native heap
copy, clone and single capture destruction against libSystem.

Remove this directory, the Cargo patch and its compatibility checks when GPUI's
cocoa/metal dependency chain stops requiring the affected `block` release or
adopts a compatible upstream fix. Do not replace this crate with `block2` by
package alias: its public API is different.
