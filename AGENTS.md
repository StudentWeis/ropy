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

Any code change in this repository — by a human or an AI agent — MUST follow the contribution SOP defined in the [`ropy-contribution-flow`](./.agents/skills/ropy-contribution-flow/SKILL.md) skill.

That skill is the single source of truth for: when to open an Issue, branch naming, Conventional Commits rules, the `scripts/precheck.sh` gate, the `gh` command templates, and the PR self-check. Read it before making any commit.
