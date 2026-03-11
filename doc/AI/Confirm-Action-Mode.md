# Confirm Action Mode

## Background

Ropy historically had a single confirmation behavior: when the user clicks an item in the main panel or presses `Enter`, the item's content is written back to the system clipboard and the window closes.

This works well for a workflow where the user copies first and pastes manually, but it doesn't suit users who want a one-step flow that both copies and immediately pastes into the previous application. This change introduces a configurable confirmation mode with two options:

- `Copy to clipboard` (default)
- `Paste immediately`

The goal is to add a faster one-step option while remaining backward compatible with existing users.

## Goals

- Add a setting in the Settings panel to switch confirmation behavior.
- Preserve the existing default behavior so upgrades do not change users' workflows.
- Make the `Paste immediately` mode reliable by ensuring the clipboard write completes before triggering a paste.
- Integrate the change with the configuration system, i18n texts and help documentation.

## Non-goals

- Do not change the behavior of the `Space` preview key.
- Do not modify how history is stored, deduplicated, or sorted.
- Do not introduce per-content-type confirmation strategies in this change; a single confirm entry point is used for all content types.

## UX Changes

The Settings panel gains a `Confirm action` control with two options:

- `Copy to clipboard`: keep the existing behavior (write to clipboard only).
- `Paste immediately`: write to the system clipboard, wait for confirmation, then automatically send a system paste shortcut to the previously focused application.

When `Paste immediately` is selected, the window's global "always-on-top" (pin) feature is disabled. The pin button in the header will be hidden and the `P` keyboard toggle will be disabled. This is deliberate: `Paste immediately` requires the window to hide and return focus to the previous application, which conflicts with an always-on-top window.

The help text for `Enter` has been adjusted to the neutral `Apply selected item` to avoid implying a single fixed behavior.

## Configuration Design

Add a `confirm` configuration section to `Settings`:

```toml
[confirm]
mode = "copy_to_clipboard"
```

`ConfirmMode` is represented by an enum with two variants:

- `copy_to_clipboard`
- `paste_immediately`

Design notes:

- Use snake_case for serialization so the configuration is human-readable.
- Default remains `copy_to_clipboard` for backward compatibility.
- Read on startup and persist changes when the Settings panel is saved.

## Runtime Flow

### Mode 1: Copy to clipboard

1. User confirms an item.
2. `confirm_record` sends a clipboard write request to the writer task.
3. The writer updates the system clipboard.
4. If the application window is not pinned, hide the window.

### Mode 2: Paste immediately

1. User confirms an item.
2. `confirm_record` sends a clipboard write request that includes a one-shot completion signal.
3. The caller waits (with timeout) for the writer to report completion.
4. Hide the Ropy window so the previously focused application can regain focus.
5. Trigger a simulated paste shortcut (platform-specific).

Step 3 is critical: if we trigger the paste before the system clipboard has been updated, the target application may paste stale content.

## Clipboard Write Completion

The clipboard writer previously accepted write requests asynchronously. For `Paste immediately` we attach an optional completion notifier to the write request:

- In normal mode nothing changes (fire-and-forget write).
- In `Paste immediately` mode the request carries a one-shot sender; the writer signals when the native clipboard write has actually completed.

This keeps changes small and minimizes impact on the existing channel design while making the immediate-paste flow deterministic. The wait timeout is currently 500ms; if we hit the timeout we log a warning and skip automatic paste to avoid incorrect behavior.

## Paste Trigger Strategy

We introduced `src/gui/paste.rs` to centralize platform paste behavior. Implementation details:

- Use `enigo` to synthesize keyboard input.
- On macOS send `Meta + V`.
- On Windows/Linux send `Control + V`.
- Sleep briefly (50ms) before sending the shortcut to allow the previously focused app to regain focus.

Centralizing paste logic makes it easier to handle platform differences and permission issues consistently.

## Internationalization

Added translation keys for:

- `settings_confirm_mode`
- `settings_confirm_mode_copy`
- `settings_confirm_mode_paste`

Also adjusted the help text `help_confirm` to be mode-agnostic.

## Files Touched

Major files changed by this feature include:

- `src/config/settings.rs`
- `src/config/mod.rs`
- `src/clipboard/mod.rs`
- `src/clipboard/writer.rs`
- `src/gui/board/mod.rs`
- `src/gui/panel/settings.rs`
- `src/gui/paste.rs`
- `assets/locales/en.toml`
- `assets/locales/zh-CN.toml`
- `assets/locales/ja.toml`
- `doc/Architecture.md`

## Validation

After implementing the change we ran the repository checks:

- `cargo test`
- `./script/precheck.sh`

Results:

- All unit tests pass.
- Precheck script passes.
- i18n key parity checks pass.

Added a small unit test to ensure `ConfirmMode` serialization and default values behave as expected.

## Known Limitations

- `Paste immediately` requires permission for synthetic input on some platforms. On macOS the app may need Accessibility (Assistive Access) privileges to send keyboard events.
- The clipboard-write completion timeout is currently hard-coded to 500ms; this may be configurable in future.
- FilePath content handling remains unmodified; the `FilePath` branch still contains existing `todo!()` behavior.

## Future Follow-ups

- Consider adding explicit user-visible feedback when automatic paste fails.
- Make the clipboard write wait timeout configurable.
- If more confirmation behaviors are needed in the future, expand `ConfirmMode` into a richer strategy enum.
