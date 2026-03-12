# Hotkey Recording Design

## Background

Ropy currently uses a plain text input in the settings panel for `hotkey.activation_key`.
The save path is:

1. Settings UI reads the string from `settings_activation_key_input`.
2. `RopyBoard::save_settings` validates it with `global_hotkey::hotkey::HotKey::from_str`.
3. If the string is accepted, it is persisted to config and sent to the running hotkey listener through `hotkey_tx`.
4. The runtime listener unregisters the old hotkey and registers the new one.

This works, but the UX is relatively weak:

- Users need to know the exact string format, such as `control+shift+d` or `cmd+shift+v`.
- Invalid input falls back to the default hotkey.
- The UI does not help the user discover valid modifier and key combinations.

The question is whether the global hotkey can be configured through a recording flow instead of manual text entry.

## Confirmed Decisions

The current discussion has already converged on three product decisions for the first implementation:

1. Keep a manual text fallback.
2. During recording, do not unregister the current global hotkey; ignore activation events instead.
3. Do not add runtime registration acknowledgement in the first iteration.

## Current Constraints

### 1. `global-hotkey` does not provide generic key recording

The current hotkey library is good at two things:

- parsing a hotkey string into `HotKey`
- registering and listening to already-registered global hotkeys

It does **not** provide a generic API for capturing arbitrary keyboard combinations for a recording UI.

So if we want a recording-style UX, the recording layer must come from somewhere else.

### 2. GPUI already exposes keyboard events inside the app window

The app already uses `on_key_down` and receives `gpui::KeyDownEvent`.
According to GPUI docs, `KeyDownEvent` contains a `Keystroke`, and `Keystroke` includes:

- `modifiers`
- `key`
- `key_char`

This means the settings window can capture a combination while the recording control has focus.

### 3. Hotkey registration has event-loop constraints

`global-hotkey` requires the manager to live on a thread with an event loop.
On macOS this must be the main thread.
Ropy already satisfies this by creating and polling the hotkey manager through the foreground executor.

This means the existing runtime registration architecture should remain unchanged.
The recording feature should feed the existing save/register pipeline instead of replacing it.

### 4. Existing runtime apply feedback is incomplete

At the moment, the settings save flow can validate the hotkey string format, but it cannot accurately confirm whether runtime registration succeeded after `hotkey_tx` sends the update.
If registration fails, the code logs a warning, but the settings UI does not receive a success or failure result.

This is still a real limitation, but it is not part of the first implementation scope.

## Feasible Solution Directions

## Option A: Focused In-App Recording

This is the recommended direction.

### Idea

Replace the free-form hotkey text entry with a recording control inside the settings panel:

- default state: show the current hotkey string
- click `Record`
- the control enters capture mode
- while focused, it listens to `KeyDownEvent` and optionally modifier changes
- when the user presses a complete combination, the UI converts it into the canonical hotkey string and stores it in the input model
- on save, reuse the existing validation and persistence flow

### Why it fits this codebase

- no new low-level keyboard hook is required
- no extra cross-platform dependency is required for the first version
- fits GPUI's existing event handling model
- keeps `global-hotkey` as the single source of truth for final validation and runtime registration

### What needs to be added

1. A small recording state machine in `RopyBoard`

Possible state:

```rust
enum HotkeyCaptureState {
    Idle,
    Recording,
    Captured(String),
}
```

In practice, `Captured(String)` is optional. `Recording + pending_display` may be enough.

2. A dedicated UI control in the settings panel

Suggested interaction:

- left side: label
- right side: read-only display area showing current or recorded combination
- `Record` button
- `Clear` button
- optional helper text: “Press the shortcut now, Esc to cancel”

3. A conversion layer from GPUI keystrokes to `global-hotkey` strings

Example output targets:

- macOS display/store: `cmd+shift+v` or `control+shift+d`
- Windows/Linux display/store: `ctrl+shift+d`

This layer should normalize modifier order and key aliases so that the saved value is stable.

4. Validation before save

Even after recording, the result should still go through:

```rust
HotKey::from_str(&recorded_hotkey)
```

This keeps the existing parser as the compatibility gate.

### Important edge cases

#### Pure modifier keys should not finish recording

If the user presses only `Shift`, `Ctrl`, `Cmd`, or `Alt`, the control should keep waiting.
The recording should finish only when there is a non-modifier main key.

#### Escape and Backspace need explicit behavior

- `Esc`: cancel recording and restore the previous value
- `Backspace` or `Delete`: clear the pending hotkey value

#### The currently active global hotkey can interfere with recording

This is the most important implementation detail.

If the existing activation hotkey remains registered while the user is recording, pressing that combination may trigger Ropy's global activation flow during recording.
Today, the activation flow can reset the settings dialog when it receives `Active`.

So recording mode should prevent the activation flow from taking effect while capture is active.

Two practical ways:

1. Temporarily unregister the runtime hotkey when recording starts, then restore it on cancel/save.
2. Keep the registration, but add a runtime guard so activation events are ignored while `show_settings && is_recording_hotkey`.

Decision: use option 2 in the first iteration.

This keeps the hotkey runtime service unchanged and limits the work to UI state and action gating.

#### Runtime registration result is still best-effort

After saving a recorded hotkey, the ideal UX would be to distinguish these outcomes:

- success: new hotkey is active
- failure: format valid, but registration failed at runtime

That would require `hotkey_tx` to evolve from fire-and-forget into a request/response shape, or be paired with an acknowledgement channel.

Decision: defer this improvement. The first implementation will keep the current best-effort apply behavior.

### Pros

- lowest implementation risk
- no new platform permissions
- minimal architecture change
- works with current settings window and save flow

### Cons

- only records keys while the Ropy settings control is focused
- key naming normalization must be implemented carefully
- some platform-specific keys may still require a fallback text edit path

## Option B: System-Level Recording Hook

### Idea

Use a low-level keyboard hook or another cross-platform crate to capture arbitrary key events at the OS level while recording.

### Advantages

- recording does not depend on the settings control focus
- more “native recorder” feel
- potentially captures combinations that GPUI may not expose consistently

### Problems

- much higher cross-platform complexity
- macOS may involve accessibility/input-monitoring permissions depending on implementation choice
- another keyboard stack would coexist with GPUI input and `global-hotkey`
- more edge cases around lifecycle, threading, and permission failures

### Assessment

This is not a good first implementation for Ropy.
The extra engineering cost is high, and the UX win over Option A is not large enough.

## Option C: Hybrid Mode

### Idea

Default to recording UI, but keep a manual text fallback for advanced users.

Possible interaction:

- regular users click `Record`
- advanced users can switch to `Manual Edit`
- final validation remains shared

### Assessment

This is a strong product option if we want the safest rollout.
It lowers the risk that uncommon keys or layout-specific cases become blocking.

If implementation scope must stay small, Option A alone is enough.
If we want better resilience, `Option A + manual fallback` is likely the best medium-term shape.

## Recommended Plan

Recommend implementing **Option A**, with one product adjustment:

> Use recording as the primary interaction, but keep a low-visibility manual fallback during the first iteration.

That gives the best balance of UX improvement and engineering safety.

### Suggested first iteration

1. Add a read-only hotkey display plus `Record` and `Clear` buttons.
2. Keep a small `Edit text` fallback for advanced users.
3. When recording starts, keep the hotkey registered, but ignore activation events while `show_settings && hotkey_recording`.
4. Capture the next valid `modifier + main key` combination from GPUI events.
5. Normalize it into the same string format already accepted by `global-hotkey`.
6. Save through the existing settings path.

Deliberately out of scope for the first iteration:

- unregistering and re-registering the current hotkey during capture
- adding runtime registration acknowledgement back to the settings UI
- changing the existing hotkey service channel shape

## Suggested Technical Shape

## UI Layer

The current settings row is built around `Input::new(&board.settings_activation_key_input)`.
For recording mode, a better structure would be:

- display chip / read-only field
- `Record` button
- `Clear` button
- optional `Edit Text` button for fallback

This likely fits naturally in `src/gui/panel/settings.rs`.

## Board State

Likely new fields in `RopyBoard`:

```rust
pub(crate) hotkey_recording: bool,
pub(crate) pending_hotkey: Option<String>,
```

The exact shape can vary, but the board needs:

- whether recording is active
- the candidate hotkey being shown
- enough information to restore the previous pending value if recording is canceled

## Conversion Utility

Add a small helper module, for example:

- `src/gui/hotkey_record.rs`

Responsibilities:

- convert `gpui::Keystroke` into canonical hotkey text
- ignore unsupported cases
- keep modifier order stable
- provide unit tests for normalization

Suggested normalization order:

- `cmd`
- `ctrl` or `control`
- `alt`
- `shift`
- main key

One detail to settle during implementation is whether to store `cmd`/`ctrl` aliases or use the exact canonical tokens preferred by `global-hotkey`.
The safest rule is:

- display user-friendly aliases
- store the exact token form that `HotKey::from_str` accepts reliably in all target platforms

## Runtime Hotkey Service

The current hotkey runtime update path uses:

- `async_channel::Sender<String>` from settings UI to listener

For the first iteration, this remains unchanged.

The listener can stay best-effort, and the recording feature can be implemented entirely on the UI side plus a small activation guard.

A later enhancement could evolve it to something like:

```rust
struct HotkeyUpdateRequest {
    hotkey: String,
    reply: async_channel::Sender<Result<(), String>>,
}
```

That would let the UI surface:

- registration success notification
- registration conflict or platform failure notification

## Testing Scope

At minimum, add unit tests for the normalization layer.

Recommended test cases:

- `ctrl + shift + d`
- `cmd + shift + v`
- modifier-only input is rejected
- escape cancels capture
- delete clears current candidate
- unusual keys such as function keys if we decide to support them

If the runtime acknowledgement path is added later, test the state transitions around successful and failed re-registration.

## Open Questions

These are the main points worth discussing before implementation:

1. What exact string format should be persisted: `cmd`/`ctrl` aliases, or another canonical token set?
2. Do we want to support function keys and media keys in the first version, or only common keyboard keys?
3. How visible should the `Edit text` fallback be in the settings UI?

## Recommendation Summary

Yes, Ropy can support a recording-style global hotkey configuration.

The practical path is:

- use GPUI key events for focused in-app recording
- keep a manual text fallback
- keep `global-hotkey` as the final validator and registrar
- ignore activation events while recording, instead of unregistering the hotkey
- keep runtime apply as best-effort in the first iteration

This is a moderate-size UI and state-management change, not a foundation rewrite.
If we control scope, it is a suitable next feature.
