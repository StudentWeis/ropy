# Memory Profiling

Ropy now includes optional `dhat` integration for Rust heap profiling.

## When to Use Which Tool

- Use `dhat` when you want Rust call stacks for heap allocations.
- Use macOS Instruments or `scripts/memory_profile.sh` when you want whole-process memory, including non-Rust UI resources.

This distinction matters for `ropy`: if memory jumps when the window is shown, some of that growth may come from AppKit, CoreAnimation, Metal, or other native allocations that are outside Rust's global allocator.

## Running DHAT

Build and run with the profiling feature enabled:

```bash
rtk cargo run --profile dhat --features dhat-heap
```

Then:

1. Launch `ropy`.
2. Reproduce the scenario you want to inspect.
3. Quit the app normally.

On exit, `dhat` writes `target/dhat-heap.json`.

Important:

- Ropy flushes the file immediately before its in-app quit actions.
- Hiding the window does not exit `ropy`.
- Prefer `Cmd-Q` or the tray's `Quit` action.
- Avoid `Ctrl-C` or force-killing the process if you want the report file.

Open the report in the online viewer:

- https://nnethercote.github.io/dh_view/dh_view.html

## What DHAT Will Show

DHAT is good at answering:

- which Rust allocation call stacks allocate the most total bytes,
- which call stacks retain the most bytes at peak,
- which allocations are still live at process exit.

For `ropy`, that can help confirm whether memory is dominated by:

- `Vec<String>` or `String` content,
- image decoding and buffering within Rust,
- UI state that is retained on the Rust heap,
- repository or cache structures.

## What DHAT Will Miss

DHAT does not explain all process memory. In particular, it will not fully capture:

- allocations before or after `main`,
- memory outside Rust's global allocator,
- macOS window backing stores,
- CoreAnimation, Metal, IOSurface, or AppKit allocations.

If `dhat` shows only modest Rust heap growth while Activity Monitor still reports a large jump, that is strong evidence the growth is in native UI or graphics memory rather than Rust heap objects.

## Recommended macOS Workflow

1. Run `rtk cargo run --profile dhat --features dhat-heap`.
2. Measure Rust heap growth with `target/dhat-heap.json`.
3. Run `bash scripts/memory_profile.sh` or use Instruments.
4. Compare the two views:

- If both are large, the issue is likely Rust-managed memory.
- If only Activity Monitor and Instruments are large, the issue is likely native window or rendering memory.
