---
name: ropy-contribution-flow
description: Standard operating procedure for contributing changes to the Ropy repository. Use whenever the user asks to implement, fix, refactor, or otherwise modify code in this repo — anything that should end up as a commit. Drives the full Issue → Branch → PR → Squash Merge flow with the local `gh` CLI, using the project's templates, Conventional Commits rules, and `scripts/precheck.sh` gate. Triggers on phrases like "add feature", "fix bug", "refactor", "implement", "create PR", "open issue", "submit change".
---

# Ropy Contribution Flow

The mandatory Issue → Branch → PR → Squash Merge workflow for any code change in this repo. General coding rules (TDD, `thiserror`, `gpui-component`, i18n, precheck) live in [`AGENTS.md`](../../../AGENTS.md) — this skill only covers the *process*.

## Skip the Issue?

Direct PR with no Issue is fine **only** for: typo / comment fixes, dependency bumps, pure `cargo fmt` output, CI tweaks. Everything else needs an Issue first.

## Conventional Commits

- **Types**: `feat | fix | docs | style | refactor | perf | test | build | ci | chore | revert`
- **Scopes** (top-level modules): `gui | repository | clipboard | updater | i18n | config | gpui`
- **Subject**: lowercase, imperative.
- The Issue title, branch type, every commit, and the PR title must all agree.

## The flow

### 1. Open the Issue

Templates: `feature_request.yml` (feature) · `bug_report.yml` (bug) · `refactor.yml` (internal cleanup).

```bash
cat > /tmp/ropy-issue.md <<'EOF'
### Motivation
<why this matters>

### Proposed Solution
<what will change>

### Acceptance Criteria
- [ ] <testable condition>

### Scope
gui   # or repository | clipboard | updater | i18n | config | gpui | other
EOF

gh issue create \
  --title "feat: <short summary>" \
  --label "enhancement" \
  --body-file /tmp/ropy-issue.md
```

Capture the returned number as `<N>`.

### 2. Branch from fresh `main`

```bash
git checkout main && git pull --ff-only
git checkout -b <type>/<N>-<kebab-slug>
# e.g. feat/42-grid-layout, fix/57-clipboard-empty-x11
```

### 3. Implement

Follow `AGENTS.md`. Re-read it if you're unsure about TDD, error types, UI components, or i18n.

### 4. Precheck must pass before committing

```bash
./scripts/precheck.sh
```

### 5. Commit

```bash
git commit -am "feat(gui): add grid layout mode

Refs #<N>"
```

Multiple commits are fine — they get squashed on merge.

### 6. Push & open the PR

```bash
git push -u origin HEAD

cat > /tmp/ropy-pr.md <<EOF
## Summary
<one or two sentences>

## Linked Issue
Closes #<N>

## Changes
- <bullet>

## Testing
- [x] \`scripts/precheck.sh\` passes locally
- [x] New / updated tests cover the change
EOF

gh pr create --base main \
  --title "feat(gui): add grid layout mode" \
  --body-file /tmp/ropy-pr.md
```

Report the PR URL back to the user.

### 7. Iterate on review

Push more commits to the same branch. **No force-push after review starts** unless the user asks.

### 8. Merge — only on explicit user instruction

```bash
gh pr merge <N> --squash --delete-branch
git checkout main && git pull --ff-only
```

The squash commit (= PR title) feeds `git-cliff` for the next CHANGELOG entry.

## CI failures

Required checks: `Precheck` and `Cross-platform build (macos-latest / windows-latest)`. Almost everything is reproducible via `./scripts/precheck.sh`. Inspect logs with `gh run view --log-failed`.
