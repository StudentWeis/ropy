<!--
PR title MUST follow Conventional Commits, e.g.:
  feat(gui): add grid layout mode
  fix(clipboard): handle empty selection on X11
Follow .agents/skills/contribution-flow/SKILL.md for the contribution SOP.
-->

## Summary

<!-- One or two sentences explaining what this PR does and why. -->

## Linked Issue

Closes #

<!-- Follow the Issue policy in .agents/skills/contribution-flow/SKILL.md.
If the user explicitly requested skipping the Issue, record that request here. -->

## Changes

<!-- Bullet list of the concrete changes made. -->
-
-

## Testing

<!-- How did you verify the change? Commands, manual steps, screenshots, etc. -->
- [ ] The selected `scripts/precheck.sh` gate passes locally
- [ ] New / updated tests cover the change

## Self-Check

- [ ] PR title follows Conventional Commits
- [ ] The selected gate passes locally: `scripts/precheck.sh`, or `scripts/precheck.sh --check --light` for Markdown-only changes
- [ ] The squash message summarizes the change instead of listing every commit subject
- [ ] No hardcoded user-facing strings — i18n keys added to all locale files
- [ ] Errors defined with `thiserror` (no manual `impl Display/Error`)
- [ ] No unrelated changes mixed in
