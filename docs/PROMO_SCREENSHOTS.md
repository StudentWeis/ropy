# Promotional screenshots

On macOS, run from the repository:

```sh
./scripts/capture-promo.sh
```

The command builds Ropy, captures its real production board in four bundled
themes, and prints the path to `ropy-themes.png`. Each run has its own directory
under `target/promo/`, containing the four original screenshots, a plain 2×2
composite, and `capture.log`. The grid order is Ropy Light / Ropy Dark on the top
row and Nord Light / Everforest Night on the bottom row. The gap and outer margin are 12 output
pixels on an opaque light gray background, so light windows remain visible on
white pages. No labels, shadows, or other decoration are added.

Requirements: an unlocked, awake macOS desktop, the repository's Rust toolchain,
Python 3, and Xcode command-line tools (`swift`). macOS must allow Screen
Recording for the terminal or app running the command. Accessibility permission
is not needed. The script checks for lock/sleep before building; keep the screen
unlocked until capture finishes. Capture currently supports macOS only; ordinary Ropy startup is
unchanged on all platforms.

```sh
# Localized demo content
./scripts/capture-promo.sh --language zh-CN

# Reuse an already built binary and choose the output directory
./scripts/capture-promo.sh --binary target/debug/ropy --output /tmp/ropy-promo
```

The screenshot process branches before normal application logging and startup.
It uses in-memory default settings, a fixed local date, fixed record IDs and
localized demo content. The Logo is extracted from the bundled asset to a
per-capture temporary directory. No personal settings or history are loaded;
clipboard monitoring/writing, global hotkeys, tray, autostart and update services
are not started. An invisible input shield keeps accidental mouse/keyboard
input from opening settings or changing the demonstration. Existing Ropy
processes can keep running.

Each window uses the application's default 400×550 logical size and list
layout. PNG dimensions follow the display's backing scale (for example,
800×1100 on a 2× display). The application signals readiness after its Logo has
loaded and a frame has rendered. The script resolves only the child process's
window, waits for consecutive identical captures, then stops that child and
cleans up its temporary data. A missing window, render failure, or unstable
capture fails within a bounded timeout. Capture errors are retained in the run's
`capture.log`; a failed run does not replace any previous output.

The composite preserves opaque screenshot pixels and blends transparent window
corners onto the gray background. Blank or differently sized
images are rejected. Fonts and rasterization may vary across macOS versions or
display scales, so this is repeatable content and geometry, not a cross-machine
pixel hash guarantee.

For debugging, the internal entry points are:

```sh
# READY_FILE's parent must exist. The file receives the child PID after rendering.
target/debug/ropy --promo ropy-light en /tmp/ropy-ready
# Stop this preview with Ctrl+C when finished.
target/debug/ropy --compose-promo /tmp/themes.png light.png dark.png nord.png forest.png
```
