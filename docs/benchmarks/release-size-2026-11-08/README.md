# Release size investigation: 2026-11-08

## Decision

Keep the existing release optimization policy (`opt-level = 3`, fat LTO, one codegen unit, panic abort, stripped symbols). Do not ship the experimental GPUI patches or mixed optimization profile. Preserve all image formats, platform integrations and accessibility in the production build.

Make the custom macOS application-update archive use explicit single-threaded XZ level 9, independent of inherited `XZ_OPT` and `XZ_DEFAULTS`. Keep the DMG format and Windows ZIP unchanged. Ensure the macOS build runner has the `xz` executable. This changes packaging, not application code or installed executable size.

The pinned cargo-dist 0.32.0 already uses XZ level 9 through axoasset 2.0.1. There is no additional Linux cargo-dist compression-level saving to claim. The custom `scripts/build_macos_dmg.sh` previously delegated compression to `tar -J` without an explicit level. This distinction supersedes the initial suggestion that all release archives could gain another 3.9% by increasing compression.

The credentials experiment is the best candidate for a future upstream optional capability. Its benefit is real, but this change does not introduce a permanent patched GUI backend to save approximately 0.51 MiB of Linux download size. Mixed optimization is also a plausible future candidate, not a rejected algorithm: its core timings are encouraging, but they do not establish GUI or cross-platform performance.

## Scope and evidence

- Issue: [#331](https://github.com/StudentWeis/ropy/issues/331).
- Measured application source: `0b35b67379bf590378a8e86a40f69d617794aabe`, Ropy 0.6.2.
- Environment: native Linux x86_64, Rust 1.99.1, GPUI Kit 0.9.0, gpui-pre 0.3.11, XZ 5.8.4.
- All size experiments use release builds. Experimental source copies come from `git archive`; Cargo's registry cache and the working checkout are not patched.
- Patched experiments necessarily change source IDs in their copied Cargo.lock. Comparison found no added package name/version pairs relative to the original lockfile. Unused packages can remain in Cargo.lock; normal production feature trees were collected separately.
- Final archives contain only an executable named `ropy`, with fixed tar metadata and `xz -T1 -9`. They are controlled local archives, not published cargo-dist packages or macOS application bundles.
- Core timing rounds run serially after compilation and compression, in opposite orders. Each round contains 47 search scenarios and three repository scenarios. Timings are Criterion median estimates, not end-to-end request P95.
- [Compressed raw measurements](measurements.json.gz) preserve sizes, hashes, compression timings, Criterion estimates and samples. Decompress with `gzip -dc measurements.json.gz`. This is a separate investigation schema, not the release benchmark v1 schema consumed by `scripts/bench.py`.
- [Experiment runner](experiment.py) and the patches below preserve the final experiment recipe. Run the recipe from a clean checkout of the measured source commit after fetching its locked dependencies. It requires the installed registry sources, Cargo, Python, GNU tar and XZ. It refuses an existing `target/size-speed/final` directory; use a fresh checkout/output location for another run. It builds disposable modified copies, not production-ready forks.

## Optimization levels

| Release optimization | Executable MiB | Result |
| --- | ---: | --- |
| `3` | 37.37 | Retained |
| `2` | 36.10 | Only 3.4% smaller; several search paths regress similarly to `s` |
| `s` | 26.26 | 29.7% smaller; requires a deliberate GUI latency tradeoff |
| `z` | 24.13 | Smaller still, but little additional download benefit over `s` and larger storage regressions |

Two repeated `2`/`3` rounds measured 10,000-record ASCII search at 1.46–1.50 ms for `3` and 6.35–6.40 ms for `2`. Long-text search measured 0.69–0.71 ms versus 4.30 ms. Unicode search and repository operations were much less affected. The earlier `s` rounds showed approximately 6.4–6.6 ms for the same ASCII workload.

Absolute latency matters: sub-millisecond workloads may remain acceptable despite a large ratio. Conversely, the benchmark excludes GUI result sorting, list updates, layout and painting. No profile is certified for interactive performance by these core benchmarks alone.

## Final isolated variants

| Variant | Executable MiB | XZ9 archive MiB | Executable reduction | Archive reduction |
| --- | ---: | ---: | ---: | ---: |
| Control | 37.37 | 9.785 | — | — |
| No credentials | 34.43 | 9.275 | 7.87% | 5.21% |
| No system notifications | 36.81 | 9.727 | 1.49% | 0.59% |
| No idle inhibition | 37.34 | 9.779 | 0.09% | 0.06% |
| Common image codecs | 35.09 | 9.374 | 6.12% | 4.20% |
| Combined removals | 31.78 | 8.813 | 14.97% | 9.94% |
| Mixed optimization | 36.02 | 9.056 | 3.62% | 7.44% |

The isolated results are not additive. Generic instantiation, shared dependencies, LTO and archive compression change the combined result.

### Credentials

[Patch](no-credentials.patch): remove the Linux backend's `oo7` dependency and replace its credential read/write/delete implementations with explicit unsupported errors. No Ropy caller was found for these APIs. The experiment removes about 2.94 MiB of executable data/code and 522 KiB of compressed data, substantially more than attributing only symbols whose names start with `oo7` would suggest.

This is the strongest upstream feature-gating opportunity. Production adoption would require a supported optional capability, explicit error semantics, upstream tests and native acceptance. It must not silently report successful credential storage.

### Notifications and idle inhibition

[Notifications patch](no-notifications.patch): remove `notify-rust` and replace the Linux notification sender with a diagnostic stub. This is a size probe, not an acceptable general-purpose notification implementation. Ropy currently uses in-app notifications; GPUI Component defaults to `NotificationDelivery::InApp`.

[Idle-inhibition patch](no-inhibit.patch): remove `ashpd/inhibit` and return an explicit unsupported error. No Ropy use was found. Savings are approximately 60 KiB and 6 KiB of compressed data respectively. They do not justify standalone permanent forks.

### Image codecs

[Patch](common-codecs.patch): remove DDS, EXR, Farbfeld, HDR, PNM, QOI, TGA and TIFF from both GPUI image dependency declarations. Retain PNG, JPEG, BMP, ICO, GIF, WebP and Rayon. The variant builds successfully and saves approximately 421 KiB in the controlled archive.

Ropy stores captured originals and thumbnails as PNG, which makes this a plausible upstream configuration. Nevertheless, removing accepted formats changes decoding capability. The experiment does not certify all clipboard sources, historic assets, platforms or previews. The formats remain enabled in production.

[Combined patch](combined.patch) applies the three platform removals plus the codec reduction. Its 9.94% archive reduction is not a compatibility pass and is not adopted.

### Mixed optimization

[Patch](mixed.patch): use `s` by default but keep `ropy`, `ropy-core`, `memchr`, `redb`, `image`, both PNG versions, `zune-jpeg` and `image-webp` at `3`. This preserves functionally identical dependencies and avoids a GUI fork, but introduces version-specific profile maintenance and changes backend code generation.

| Workload | Control, two rounds | Mixed, two rounds |
| --- | --- | --- |
| ASCII hit, 10,000 records | 1.543 / 1.524 ms | 1.526 / 1.538 ms |
| Insert | 38.48 / 38.89 µs | 39.09 / 38.31 µs |
| Deduplicate | 42.37 / 42.53 µs | 43.22 / 42.32 µs |
| Read 100 records | 110.75 / 93.85 µs | 106.21 / 96.14 µs |

Across the 50 workloads, the median of the per-workload ratios of averaged round medians was 1.003. Individual ratios ranged from 0.914 to 1.099. This descriptive calculation is not a statistical equivalence test. It avoids the large core-search regression of global `s`, but does not measure GPUI startup, text layout, rendering, input, image processing, or IPC latency.

Native Linux GUI comparison was not performed: this host did not have Xvfb, xdotool, Openbox or xclip available. No host clipboard or everyday Ropy instance was used as a substitute. macOS, Windows and Linux ARM64 were not measured. Rather than infer GUI safety from core tests, keep the existing production profile.

## Compression experiment

One fixed tar input per binary, one compression thread, three warm decompression runs:

| Binary / preset | Archive bytes | Compression seconds | Decompression seconds |
| --- | ---: | ---: | --- |
| `3`, XZ6 | 10,678,580 | 13.16 | 0.318–0.322 |
| `3`, XZ9 | 10,260,396 | 14.58 | 0.316–0.317 |
| `3`, XZ9 extreme | 10,260,428 | 15.61 | 0.316–0.320 |
| `2`, XZ6 | 10,319,244 | 12.61 | See raw measurements |
| `2`, XZ9 | 9,949,696 | 14.07 | See raw measurements |
| `2`, XZ9 extreme | 9,945,224 | 15.09 | See raw measurements |

For `3`, XZ9 saves 408 KiB (3.9%) relative to XZ6. Measured compression peak RSS increases from approximately 96 MiB to 408 MiB; decompression peak RSS increases from approximately 15 MiB to 41 MiB. This does not change application runtime memory or execution speed. Extreme mode provides no useful additional saving.

The final control archive differs slightly from the earlier compression sample because it was rebuilt in a separate source path. Compare variants within their experiment, not individual byte counts across stages.

The current cargo-dist compression default was verified against its [v0.32.0 lockfile](https://github.com/axodotdev/cargo-dist/blob/v0.32.0/Cargo.lock) and [axoasset 2.0.1 compression implementation](https://docs.rs/crate/axoasset/2.0.1/source/src/compression.rs). Its XZ encoder defaults to level 9. The 3.9% experiment therefore motivates aligning the custom macOS update archive, not claiming another 3.9% reduction of existing Linux releases. Actual macOS package savings have not been measured on a Mac.

## D-Bus capability audit

| Capability | Current usage | Decision |
| --- | --- | --- |
| StatusNotifier tray/menu (`ksni`) | Explicit Ropy functionality on Linux | Keep |
| Global shortcuts (`ashpd/global_shortcuts`) | Selected Wayland registration path; Hyprland has its own path | Keep |
| File chooser | Theme export calls `prompt_for_new_path` | Keep |
| Open URI | Used by links/directory opening, with a portal fallback | Keep |
| Desktop settings | GPUI installs appearance/cursor/button-layout listeners | Keep |
| Reduced motion | Base initialization follows the portal setting | Keep |
| AT-SPI accessibility | Window initialization installs the adapter; Ropy supplies accessible labels | Keep |
| Sleep/wake | GPUI registers login1 system-bus callbacks during app initialization | Keep |
| Power mode | GPUI registers power-profile monitoring during app initialization | Keep |
| Credentials | No application call found, implementation retained in binary | Upstream optional-feature candidate |
| System notifications | Ropy currently uses in-app delivery | Do not fork for small savings |
| Idle inhibition | No application call found | Do not fork for negligible savings |

The current Linux backend has no independent production feature switches for credentials or system notifications. Accessibility is enabled with the X11/Wayland backends. Upstream-enabled Cargo features cannot be disabled by the application's `default-features = false`. Runtime disabling also does not guarantee that trait-object implementations or generic code disappear from the executable.

This is a source and linked-symbol audit, not a trace of a user's session bus. The existence of a compiled capability does not mean it connects at startup: credentials connect when called; appearance/power listeners are registered automatically. The current Enigo features do not enable its libei/RemoteDesktop portal implementation.

## Verification boundaries

The packaging tests exercise the actual tar/XZ payload and checksum with stubbed Cargo and DMG commands. They also assert explicit compression options, isolation from inherited compressor settings, and a nonzero exit without a new checksum when compression fails. Both new protections were observed failing before the script change and passing afterward.

The release workflow is regenerated using the pinned cargo-dist 0.32.0, not the newer globally installed version. No generated workflow is hand-edited. Local validation passed: full `scripts/precheck.sh --check` (resources, script tests, Clippy, workspace all-target/all-feature tests, private-session-bus tray regression and Rust documentation), pinned `dist generate --mode ci --check`, pinned `dist plan`, shell syntax/formatting, Markdown checks and raw-evidence integrity checks. CI results belong to the implementation PR. These checks do not convert the experimental GUI variants into release-certified builds.

This decision deliberately avoids unmeasured performance claims, removal of supported desktop integrations, and a permanent vendor tree. It leaves a reproducible upstream optimization proposal instead of shipping the exploratory stubs.
