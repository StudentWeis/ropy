---
name: ropy-contribution-flow
description: Ropy repository contribution workflow. Use for any code change that should become a commit — adding features, fixing bugs, refactoring, or any source modification. Drives the full Issue → Branch → PR → Merge process.
---

# Ropy Contribution Flow

Issue → Branch → PR → Merge workflow for any code change in this repo. Coding standards live in [`AGENTS.md`](../../../AGENTS.md); this skill only covers the process.

## Conventional Commits

- **Types**: `feat | fix | docs | style | refactor | perf | test | build | ci | chore | revert`
- **Scopes** (top-level modules): `gui | repository | clipboard | updater | i18n | config | gpui`
- **Subject**: lowercase, imperative mood, no trailing period.
- **Consistency**: the `<type>` must match across the Issue title, branch prefix, every commit, and the PR title. The `<scope>` is required on commits and the PR title, optional on the Issue title, and omitted from the branch name.

## The flow

### 1. Open the Issue

Pick a template from `.github/ISSUE_TEMPLATE/`. Fill it into `/tmp/ropy-issue.md`, then:

```bash
gh issue create \
  --title "<type>: ..."  \
  --label "<from yml>" \
  --body-file /tmp/ropy-issue.md
```

Capture the returned number as `<N>`.

> **Skipping the Issue**: only when the user explicitly asks. Proceed to Step 2 with a descriptive branch name and note the skip in the PR description.

### 2. Branch from fresh `main`

```bash
git checkout main && git pull --ff-only
git checkout -b <type>/<kebab-slug>
# e.g. feat/grid-layout, fix/clipboard-empty-x11
```

### 3. Implement

Follow `AGENTS.md`. Re-read it if unsure about TDD, error types, UI components, or i18n.

### 4. Precheck

Must pass before any commit:

```bash
./scripts/precheck.sh
```

### 5. Commit

```bash
git commit -am "feat(gui): add grid layout mode

Refs #<N>"
```

Multiple commits are fine — they are squashed on merge.

### 6. Push & open the PR

Fill `.github/PULL_REQUEST_TEMPLATE.md` into `/tmp/ropy-pr.md`, then:

```bash
git push -u origin HEAD
gh pr create --base main \
  --title "<type>(<scope>): ..." \
  --body-file /tmp/ropy-pr.md
```

Report the PR URL back to the user.

### 7. Iterate on review

Push additional commits to the same branch. **Do not force-push once a reviewer has left feedback**, unless the user explicitly requests it.

### 8. Merge — only on explicit user instruction

```bash
gh pr merge <N> --squash --delete-branch
git checkout main && git pull --ff-only && git fetch --prune
```

## CI failures

Required checks: `Precheck` and `Cross-platform build (macos-latest / windows-latest)`. Most failures are reproducible locally via `./scripts/precheck.sh`. Inspect logs with `gh run view --log-failed`.
