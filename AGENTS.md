## General Principles

- Reason from first principles — question assumptions before adopting patterns.
- Follow Clean Code, DRY, and KISS — favor clarity over cleverness.
- Don't over-abstract and keep it simple.
- Practice TDD: write a failing test first, then implement the minimal code to make it pass, then refactor.

## Development

- Follow all the best practices outlined in the clippy lint.
- Define all error types with `thiserror` — avoid manual `impl Display/Error`.
- Build new UI components using `gpui-component`.
- Localize all user-facing strings — no hardcoded display text in source files.
- Follow the guidelines in [Testing](./docs/TESTING.md) for test structure, naming, and coverage expectations.
- Log files can be viewed when troubleshooting problems.

## AI Collaboration Workflow

Any code change in this repository — by a human or an AI agent — MUST follow the contribution SOP defined in the [`contribution-flow`](./.agents/skills/contribution-flow/SKILL.md) skill.

That skill is the single source of truth for: when to open an Issue, branch naming, Conventional Commits rules, the `scripts/precheck.sh` gate, the `gh` command templates, and the PR self-check. Read it before making any commit.

## GPUI Skills

For Ropy UI development, review, testing and GPUI upgrades, use the official [`gpui-kit`](./.agents/skills/gpui-kit/SKILL.md) skill together with the project [`ropy-gpui`](./.agents/skills/ropy-gpui/SKILL.md) adapter. For visible UI work, also use the official [`gpui-kit-design-guides`](./.agents/skills/gpui-kit-design-guides/SKILL.md) skill.

Keep both official skill directories identical to the upstream revision recorded in `ropy-gpui`. Put every Ropy-specific addition, correction, version constraint and verification instruction in `ropy-gpui`; do not modify the official copies. Resolve API differences against the installed source and Cargo.lock, and apply repository conventions through the adapter.
