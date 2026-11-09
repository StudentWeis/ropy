# Ropy documentation

Start with the [English README](../README.md) or [中文 README](../README.zh-CN.md) to install Ropy and use clipboard history. This index covers documentation maintained with the source; a pre-built release may not include every source change. Check the [changelog](../CHANGELOG.md) for shipped changes.

## Use and customize Ropy

| Task | Guide |
| --- | --- |
| Find settings, change confirmation mode, or recover an unreadable configuration | [Configuration and recovery](configuration.md) |
| Change history shortcuts or remove an alias | [History shortcuts](keybindings.md) |
| Create, import, edit or reload a personal theme | [Custom themes](themes.md) |
| Set up native Wayland activation and understand compositor limits | [Native Wayland support](wayland.md) |
| Build or maintain the Arch binary package | [AUR packaging](../packaging/aur/README.md) |

## Contribute and understand the implementation

| Task | Guide |
| --- | --- |
| Set up a checkout and run development commands | [Development](development.md) |
| Contribute bundled themes, translations or code | [Contributing](../CONTRIBUTING.md) / [中文贡献指南](../CONTRIBUTING.zh-CN.md) |
| Follow the Issue, branch, commit and PR procedure | [Contribution flow](../.agents/skills/contribution-flow/SKILL.md) |
| Understand package boundaries, persistence and background tasks | [Architecture](architecture.md) |
| Choose tests, reproduce CI, or collect native regression evidence | [Testing](testing.md) |
| Capture isolated promotional screenshots on macOS | [Promotional screenshots](promo-screenshots.md) |
| Maintain documentation and verify its facts | [Project documentation skill](../.agents/skills/project-doc/SKILL.md) |

## Validate and release

| Task | Guide |
| --- | --- |
| Maintain generated release jobs, updater recovery or Homebrew publishing | [Release workflow maintenance](releasing.md) |
| Accept exact package bytes on disposable desktops | [Release package acceptance](release-acceptance.md) |
| Run native GUI tests with platform isolation | [Cross-platform GUI procedure](../.agents/skills/cross-platform-gui-e2e-test/SKILL.md) |
| Reproduce the Sway and portal regressions in a Linux VM | [Local Wayland regression on macOS](wayland-vm.md) |
| Understand the Windows RDP geometry mitigation and its unverified boundary | [Windows RDP popup DPI restore](windows-rdp-dpi.md) |

Passing unit tests, protocol fixtures or a development build does not certify a distributed package. Record platform, source revision, artifact digest and untested scenarios as required by the acceptance guide.

## Measure performance and read historical evidence

- [Release benchmark baseline](benchmarks.md): run storage, executable-size and isolated macOS RSS measurements; retain comparable results.
- [Search benchmark and implementation](search-performance.md): matching contracts, workload, selected algorithm and reproduction commands.
- [Benchmark evidence index](benchmarks/README.md): dated measurements and release-size decisions. Historical numbers are not measurements of the current checkout.
- [Changelog](../CHANGELOG.md): generated release history, not a task list.
