<!--
PR title MUST follow Conventional Commits, e.g.:
  feat(gui): add grid layout mode
  fix(clipboard): handle empty selection on X11
CI will reject the PR otherwise.
-->

## Summary

<!-- One or two sentences explaining what this PR does and why. -->

## Linked Issue

Closes #

<!-- For trivial changes (typo, deps bump, formatting, CI tweak) write "N/A — trivial change" instead. -->

## Changes

<!-- Bullet list of the concrete changes made. -->
-
-

## Testing

<!-- How did you verify the change? Commands, manual steps, screenshots, etc. -->
- [ ] `scripts/precheck.sh` passes locally
- [ ] New / updated tests cover the change

## Self-Check

- [ ] PR title follows Conventional Commits
- [ ] No hardcoded user-facing strings — i18n keys added to all locale files
- [ ] Errors defined with `thiserror` (no manual `impl Display/Error`)
- [ ] No unrelated changes mixed in
