# Release size decision: 2026-11-08

## Adopted change

Keep the existing release optimization policy and dependencies. Use explicit single-threaded XZ level 9 for the custom macOS application-update archive, ignoring inherited `XZ_OPT` and `XZ_DEFAULTS`. Preserve the archive contents, checksum, DMG format and Windows ZIP behavior. Ensure the macOS release runner has the `xz` executable.

The pinned cargo-dist 0.32.0 already uses XZ level 9 through axoasset 2.0.1. There is no additional Linux cargo-dist compression-level saving to claim. The custom `scripts/build_macos_dmg.sh` previously used `tar -J` without an explicit level.

## Measurements informing the decision

Measured source: `0b35b67379bf590378a8e86a40f69d617794aabe` (Ropy 0.6.2), native Linux x86_64, Rust 1.99.1, GPUI Kit 0.9.0 and gpui-pre 0.3.11. These are local binary-only archives, not published packages or macOS application bundles. Experiments and raw data remain outside the tracked source tree under `target/size-speed/`; the experimental variants below were not adopted.

| Experimental variant | Executable MiB | Fixed-metadata XZ9 archive MiB | Archive reduction |
| --- | ---: | ---: | ---: |
| Current configuration | 37.37 | 9.785 | — |
| No credentials backend | 34.43 | 9.275 | 5.21% |
| No system notifications | 36.81 | 9.727 | 0.59% |
| No idle inhibition | 37.34 | 9.779 | 0.06% |
| Reduced image codecs | 35.09 | 9.374 | 4.20% |
| Combined removals | 31.78 | 8.813 | 9.94% |
| Mixed optimization | 36.02 | 9.056 | 7.44% |

The mixed profile used `s` for most dependencies and `3` for the application, core, search, database and selected image dependencies. Two opposite-order rounds of 50 core benchmarks avoided the large search regressions seen with global `2`/`s`/`z`. However, core benchmarks do not measure GUI startup, rendering, input or cross-platform performance. Global `2` reduced executable size by only 3.4% while increasing the measured 10,000-record ASCII search from approximately 1.48 ms to 6.37 ms.

For the same fixed tar input and single compression thread, XZ9 reduced the current binary's archive from 10,678,580 bytes to 10,260,396 bytes compared with XZ6: approximately 408 KiB or 3.9%. Compression took 13.16 versus 14.58 seconds; warm decompression remained approximately 0.32 seconds. Peak compression RSS increased from approximately 96 to 408 MiB, and decompression RSS from approximately 15 to 41 MiB. XZ extreme mode provided no useful additional saving. This comparison does not establish actual macOS package savings or further Linux release savings.

## Rejected production changes

- Do not maintain a private GPUI fork for these savings. Optional credentials support is a useful upstream proposal, but the current backend has no independent switch. Notification and idle-inhibition savings are too small to justify standalone forks.
- Do not remove supported image formats without clipboard and preview compatibility evidence. The codec experiment removed DDS, EXR, Farbfeld, HDR, PNM, QOI, TGA and TIFF; production support remains unchanged.
- Do not adopt mixed optimization solely from core timings. Native GUI and other-platform performance were not measured.
- Keep D-Bus integration for the tray, Portal shortcuts, theme export, URI fallback, desktop preferences, reduced motion, accessibility and power monitoring. Unused credentials APIs do not make the shared D-Bus stack removable.

## Verification

Packaging regressions exercise real tar/XZ payloads and checksums with stubbed Cargo and DMG commands. They protect explicit compressor options, isolation from inherited settings and failure before checksum generation. The new assertions failed before the change and passed afterward.

Full local `scripts/precheck.sh --check`, pinned cargo-dist generation and plan checks passed. The initial implementation also passed Linux, macOS and Windows CI checks. Those builds/tests do not certify native macOS packaging or experimental-profile GUI performance. No experimental source patch, profile override or vendor tree is part of the production change.

See [issue #331](https://github.com/StudentWeis/ropy/issues/331) and [PR #332](https://github.com/StudentWeis/ropy/pull/332).
