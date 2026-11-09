# Development

Use this guide to prepare a source checkout and run local commands. For patch submission, follow [contribution-flow](../.agents/skills/contribution-flow/SKILL.md); for test selection and CI, see [Testing](TESTING.md). Commands below run from the repository root.

## Prerequisites

- Install Rust with rustup. The stable compiler, rustfmt and Clippy components are pinned in [rust-toolchain.toml](../rust-toolchain.toml); run `rustup show active-toolchain` in the checkout to install them.
- Install Python 3.11+ for resource checks and script tests.
- Make targets require GNU Make, Bash and `awk`. On Windows, use a compatible shell such as MSYS2; native Windows builds still require the Windows toolchain.
- Install the native build dependencies for your platform. Linux's maintained package list is `[dist.dependencies.apt]` in [dist-workspace.toml](../dist-workspace.toml). Linux precheck also requires `dbus-run-session` (Ubuntu package `dbus`).
- macOS bundling and promotional capture require Xcode command-line tools. Windows resource generation requires a Windows host; cross-OS builds with a Windows host or target are rejected.

## Build and run

```sh
git clone https://github.com/StudentWeis/ropy.git
cd ropy
cargo build --locked --release
./target/release/ropy
```

The final path assumes Cargo's default native target directory. If you configure a target or output directory, launch the executable from that output instead. Normal startup accesses your user configuration and clipboard history; use the [isolated GUI procedure](../.agents/skills/cross-platform-gui-e2e-test/SKILL.md) for testing.

Ropy starts hidden. Open it with the activation shortcut or tray; Linux also supports `ropy --show`. Native Wayland has compositor-specific [activation and clipboard requirements](WAYLAND.md).

## Command entry points

Run `make` or `make help` for the available targets. [Makefile](../Makefile) owns the exact commands.

| Command | Purpose |
| --- | --- |
| `make build` / `make build-release` | Build the default desktop package in debug / release mode |
| `make run` | Build and launch the debug application |
| `make check` / `make clippy` | Check or lint all workspace targets and features |
| `make test` | Run workspace tests for all targets and features |
| `make fmt` / `make fmt-check` | Format or check Rust across both packages |
| `make doc` | Generate workspace rustdoc with warnings treated as errors |
| `make precheck` | Format locally, then run the complete pre-commit gate |
| `make clean` | Remove Cargo build artifacts |

Direct Cargo targets support overrides such as `make build CARGO="rtk cargo"`; script targets retain their own command selection. The root desktop package is the default Cargo member. Use explicit `--workspace` for both packages or `-p ropy-core` for portable core work; see [Architecture](ARCHITECTURE.md).

For performance commands and measurement boundaries, use [release benchmarks](BENCHMARKS.md) and [search benchmarks](SEARCH_PERFORMANCE.md). Do not run timing comparisons alongside compilation.

## Optional tool and hook setup

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) before running:

```sh
make setup
```

[scripts/init.sh](../scripts/init.sh) installs cargo-binstall, hk 2.5+, the configured cargo-dist version, cargo-release, git-cliff and cargo-outdated. It replaces the existing pre-commit hook; review the script before running it in a checkout with custom hooks.

YAML, TOML and JSON hooks use `uvx` with pinned parsers. Run `hk check --all` to check files or `hk fix --all` to fix them. Hooks leave fixes unstaged and fail the commit so you can review and stage them before retrying. The official GPUI skill snapshots are excluded from formatting fixes.

## Before submitting

Run `./scripts/precheck.sh`; it is mandatory before a commit, including documentation-only commits. Use `--check` for non-mutating CI reproduction. The light and Rust phases, required CI checks and native tests are described in [Testing](TESTING.md).

For Markdown, also review local links, anchors, code fences and `git diff --check`. Precheck's documentation step builds Rust API documentation; it does not validate Markdown links or establish native GUI acceptance. Documentation ownership and evidence requirements live in [project-doc](../.agents/skills/project-doc/SKILL.md).
