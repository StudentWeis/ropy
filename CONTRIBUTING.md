# Contributing to Ropy

English | [简体中文](docs/CONTRIBUTING/CONTRIBUTING_ZH.md)

Use this guide to contribute code, bundled themes, translations or documentation. For a personal palette that does not ship with Ropy, use [Custom themes](docs/THEMES.md) instead.

## Prepare a change

1. Follow [contribution-flow](.agents/skills/contribution-flow/SKILL.md) for the required Issue → Branch → PR → Squash Merge procedure. It owns templates, branch and commit naming, precheck and merge authorization.
2. Prepare the toolchain and native dependencies with [Development](docs/DEVELOPMENT.md). Read [AGENTS.md](AGENTS.md) for coding conventions and [Architecture](docs/ARCHITECTURE.md) for module boundaries.
3. Choose tests using [Testing](docs/TESTING.md). Run `./scripts/precheck.sh` before committing; resource checks alone do not replace the complete gate.

## Add a bundled theme

Bundled themes live in [assets/themes](assets/themes/). Create a complete `.toml` file using a lowercase, hyphen-separated filename such as `my-theme.toml`; the filename without its extension becomes the built-in theme ID.

Start from [ropy-dark.toml](assets/themes/ropy-dark.toml) or [ropy-light.toml](assets/themes/ropy-light.toml). Include a nonempty `theme_name`, `mode` (`light` or `dark`) and every color field. Use six-digit hexadecimal RGB values and follow the template rather than maintaining a separate field list.

Run `python3 scripts/check/check_themes.py`, then launch an isolated app build to check legibility, selected/hover states and both list and grid layouts. The resource check does not establish visual quality. UI changes also follow the [GPUI skills](AGENTS.md#gpui-skills).

## Add or update a language

Language files live in [assets/locales](assets/locales/) and use TOML. Copy the structure and keys from [en.toml](assets/locales/en.toml). Use a [BCP 47](https://www.rfc-editor.org/rfc/bcp/bcp47.txt) language code for the filename, such as `fr.toml`, `ko.toml` or `pt-BR.toml`.

Set `language_name` to the language's own name for the settings selector. Keep translations concise and consistent, preserve interpolation names such as `%{count}`, and check the actual interface after rebuilding. Resource files are embedded at compile time; adding a language file does not require a separate language list.

Application code calls `rust_i18n::t!` directly with English fallback. Dynamic translation-key fields end in `_key` so the checker can identify their literals. Only startup and a successfully saved language change set the shared locale; tests and offline fixtures use an explicit locale. Do not add another translation map or locale global.

Run `python3 scripts/check/check_i18n.py` to check TOML, key parity and interpolation names. See [Architecture](docs/ARCHITECTURE.md#dependency-direction) for runtime localization ownership.

## Maintain documentation

Read [project-doc](.agents/skills/project-doc/SKILL.md) before writing or reorganizing project documentation. Use the [documentation index](docs/README.md) to choose the existing owner; keep the English and Chinese README/contribution entry points aligned. Do not hand-edit generated benchmark results or rewrite historical measurements as current validation.

## Releases and issue reports

Maintainers use [Release workflow maintenance](docs/RELEASING.md#protected-release-sequence) for version preparation and publishing, and [Release package acceptance](docs/RELEASE_ACCEPTANCE.md) for exact-artifact GUI evidence. Preparing documentation or a PR does not authorize publishing or merging.

Report bugs and feature suggestions through the matching [Issue template](https://github.com/StudentWeis/ropy/issues/new/choose). Include the platform, version, reproduction steps and relevant logs without personal clipboard content.
