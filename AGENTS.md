## Total

- Think in terms of first principles.
- Write code following the principles of Clean Code, DRY and KISS.
- Follow the TDD (Test-Driven Development) approach, prioritizing writing tests that fail first, then writing implementation code that passes.

## Development

- Use thiserror to define error types.
- Use gpui-component to add new UI components.
- Use scripts/precheck.sh to check code quality and format.

## Tests

- Use tempfile to create temporary files.
- Use rstest to perform parameterized tests.
- Use appropriate macros to avoid clippy warnings in test functions.
- Test functions should follow the `test_<object>_<scenario>_<expected>` naming pattern.

## Others

- Use the [rtk](./RTK.md) prefix when executing Shell commands.
