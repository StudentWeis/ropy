---
name: ropy-contribution-flow
description: Standard operating procedure for contributing changes to the Ropy repository. Use whenever the user asks to implement, fix, refactor, or otherwise modify code in this repo — anything that should end up as a commit. Drives the full Issue → Branch → PR → Squash Merge flow with the local `gh` CLI, using the project's templates, Conventional Commits rules, and `scripts/precheck.sh` gate. Triggers on phrases like "add feature", "fix bug", "refactor", "implement", "create PR", "open issue", "submit change".
---

# Ropy Contribution Flow

This skill encodes the project's mandatory workflow for any code change. Follow it whenever the user requests a change to the Ropy codebase.

## When to use this skill

- The user asks for any code modification (feature, fix, refactor, docs of substance).
- The user explicitly asks you to "open an issue / PR" or "follow the contribution flow".

## When NOT to use this skill (skip Issue, optionally skip PR ceremony)

A direct PR with no Issue is acceptable for **trivial changes only**:

- Typo / comment fixes
- Dependency version bumps
- Pure formatting (`cargo fmt` results)
- CI / workflow tweaks

Anything else — including non-trivial docs — needs an Issue.

## Required tools

- `gh` CLI (logged in to `StudentWeis/ropy`)
- `git`
- A working `scripts/precheck.sh` (Rust toolchain pinned by `rust-toolchain.toml`)

## The flow

Execute these steps in order. Do not skip steps. Do not batch them mentally — actually run each command and verify output before moving on.

### 1. Clarify scope, then create the Issue

Pick the matching template:

| Change kind | Template file |
|---|---|
| New feature / enhancement | `feature_request.yml` |
| Bug fix | `bug_report.yml` |
| Internal cleanup, no user-visible change | `refactor.yml` |

Draft the body in a temp file (avoids quoting hell):

```bash
cat > /tmp/ropy-issue.md <<'EOF'
### Motivation
<why this matters>

### Proposed Solution
<what will change>

### Acceptance Criteria
- [ ] <testable condition 1>
- [ ] <testable condition 2>

### Scope
gui   # or repository | clipboard | updater | i18n | config | gpui | other
EOF

gh issue create \
  --title "feat: <short summary>" \
  --label "enhancement" \
  --body-file /tmp/ropy-issue.md
```

> The title MUST be a valid Conventional Commit. The same title will become the PR title later.

Capture the returned issue number as `<N>`.

### 2. Branch from a fresh `main`

```bash
git checkout main
git pull --ff-only
git checkout -b <type>/<N>-<kebab-slug>
# examples: feat/42-grid-layout   fix/57-clipboard-empty-x11   refactor/63-repo-backend-split
```

### 3. Implement (TDD when feasible)

Per `AGENTS.md` general principles:

- Write a failing test first when behavior is testable.
- Define error types with `thiserror`.
- Build new UI with `gpui-component`.
- All user-facing strings go through i18n keys in **every** locale file under `assets/locales/` — never hardcode display text.

### 4. Run precheck — it must pass before committing

```bash
./scripts/precheck.sh
```

This runs nightly `cargo fmt`, `cargo check`, `cargo clippy --all-targets --all-features`, `cargo test -- --test-threads=1`, and the i18n / icons / themes Python checks. Fix everything it complains about; do not commit a red precheck.

### 5. Commit with Conventional Commits + Issue footer

```bash
git add -A
git commit -m "feat(gui): add grid layout mode

Refs #<N>"
```

**Allowed types**: `feat | fix | docs | style | refactor | perf | test | build | ci | chore | revert`
**Allowed scopes** (match top-level modules): `gui | repository | clipboard | updater | i18n | config | gpui`

Multiple commits on the branch are fine — they will be squashed on merge.

### 6. Push and open the PR

```bash
git push -u origin HEAD

cat > /tmp/ropy-pr.md <<EOF
## Summary
<one or two sentences>

## Linked Issue
Closes #<N>

## Changes
- <bullet 1>
- <bullet 2>

## Testing
- [x] \`scripts/precheck.sh\` passes locally
- [x] New / updated tests cover the change

## Self-Check
- [x] PR title follows Conventional Commits
- [x] No hardcoded user-facing strings — i18n keys added to all locale files
- [x] New UI components use \`gpui-component\`
- [x] Errors defined with \`thiserror\`
- [x] No unrelated changes mixed in
EOF

gh pr create \
  --base main \
  --title "feat(gui): add grid layout mode" \
  --body-file /tmp/ropy-pr.md
```

Report the PR URL back to the user.

### 7. Wait for review; iterate by pushing more commits

- More commits on the same branch are fine — squash merge collapses them.
- **Do not force-push after review starts** unless the user explicitly asks.
- For each round of feedback, re-run `scripts/precheck.sh` before pushing.

### 8. Merge — only on explicit user instruction

```bash
gh pr merge <N> --squash --delete-branch
```

The squash commit message defaults to the PR title (a Conventional Commit), which `git-cliff` will pick up for the next CHANGELOG entry. After merge, switch back and pull:

```bash
git checkout main
git pull --ff-only
```

## Pre-PR self-check (verify before step 6)

- `scripts/precheck.sh` passes locally.
- No hardcoded user-facing strings; i18n keys added in **all** locale files (`en.toml`, `zh-CN.toml`, `ja.toml`, ...).
- New UI uses `gpui-component`; no raw `div()` reinventions of existing components.
- Errors defined with `thiserror`; no manual `impl Display/Error`.
- Tests added or updated when behavior changed.
- PR title is a valid Conventional Commit (`<type>(<scope>): <lowercase subject>`).
- PR body contains `Closes #<N>` (or, for trivial changes, `N/A — trivial change`).

## Handling CI failures

CI runs on PR open and on every push to the PR branch. The required checks are:

- `Static checks (fmt + i18n + themes + icons)`
- `Build & Test (ubuntu-22.04 / macos-latest / windows-latest)`
- `Validate Conventional Commit title`

If a check fails:

1. Open the failing job: `gh run view --log-failed`
2. Reproduce locally where possible (most failures are reproducible via `scripts/precheck.sh`).
3. Fix, re-run precheck, push. Do not skip checks.

## Anti-patterns — never do these

- ❌ Pushing directly to `main`.
- ❌ Opening a PR without an Issue (unless the change qualifies as trivial — see top of skill).
- ❌ Merge commits on `main` (squash only).
- ❌ Commit messages that are not Conventional Commits.
- ❌ Hardcoding user-facing strings instead of using i18n keys.
- ❌ Force-pushing to a PR branch after review has started.
- ❌ Marking precheck-failing code as "ready for review".

## Quick reference

```bash
# Full happy path, copy-paste-ready
gh issue create --title "feat: ..." --body-file /tmp/ropy-issue.md   # → captures #N
git checkout main && git pull --ff-only
git checkout -b feat/<N>-<slug>
# ...edit code...
./scripts/precheck.sh
git commit -am "feat(<scope>): <subject>

Refs #<N>"
git push -u origin HEAD
gh pr create --base main --title "feat(<scope>): <subject>" --body-file /tmp/ropy-pr.md
# ...review iteration...
gh pr merge <N> --squash --delete-branch
```
