## General Principles

- Reason from first principles — question assumptions before adopting patterns.
- Follow Clean Code, DRY, and KISS — favor clarity over cleverness.
- Don't over-abstract and keep it simple.
- Practice TDD: write a failing test first, then implement the minimal code to make it pass, then refactor.
- Prefix all shell commands with [rtk](./doc/RTK.md).

## Development

- Define all error types with `thiserror` — avoid manual `impl Display/Error`.
- Build new UI components using `gpui-component`.
- Run `scripts/precheck.sh` before committing to verify code quality and formatting.
- Localize all user-facing strings — no hardcoded display text in source files.
- Follow the guidelines in [Testing](./doc/TESTING.md) for test structure, naming, and coverage expectations.
