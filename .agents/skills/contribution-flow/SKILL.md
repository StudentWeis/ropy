---
name: contribution-flow
description: Repository contribution workflow. Use for any code change that should become a commit — adding features, fixing bugs, refactoring, or any source modification. Drives the full Issue → Branch → PR → Merge process.
---

# Contribution Flow

Issue → Branch → PR → Merge workflow for any code change in this repo. Coding standards live in the project's development guidelines (e.g. `AGENTS.md`); this skill only covers the process.

## Conventions

Shared vocabulary used throughout the flow below: `<type>` and `<scope>` placeholders in every step refer back here.

- **Types**: `feat | fix | docs | style | refactor | perf | test | build | ci | chore | revert`
- **Scopes**: infer from the project's top-level module/directory structure (e.g. `core`, `api`, `ui`).
- **Subject**: lowercase, imperative mood, no trailing period.
- **Consistency**: the `<type>` must match across the Issue title, branch prefix, every commit, and the PR title. The `<scope>` is required on commits and the PR title, optional on the Issue title, and omitted from the branch name.

## The flow

### 1. Open the Issue

If the project has issue templates (`.github/ISSUE_TEMPLATE/`), pick the appropriate one. Otherwise use a concise description with acceptance criteria. Fill it into `/tmp/issue-body.md`, then:

```bash
gh issue create --title "<type>: ..." --label "<appropriate label>" --body-file /tmp/issue-body.md
```

Capture the returned number as `<N>`.

> **Skipping the Issue**: only when the user explicitly asks. Proceed to Step 2 with a descriptive branch name and note the skip in the PR description.

### 2. Branch from fresh `main`

```bash
git checkout main && git pull --ff-only
git checkout -b <type>/<kebab-slug> # e.g. feat/grid-layout, fix/clipboard-empty-x11
```

### 3. Implement

Follow the project's development guidelines.

### 4. Precheck

Must pass before any commit:

```bash
./scripts/precheck.sh
```

### 5. Commit

```bash
git commit -am "<type>(<scope>): <short description>

Refs #<N>"
```

Multiple commits are fine — they are squashed on merge.

### 6. Push & Open the PR

If the project has a PR template (`.github/PULL_REQUEST_TEMPLATE.md`), fill it into `/tmp/pr-body.md`. Otherwise write a concise summary with a test plan. Then:

```bash
git push -u origin HEAD
gh pr create --base main --title "<type>(<scope>): ..." --body-file /tmp/pr-body.md
```

Report the PR URL back to the user.

### 7. Iterate on review

Push additional commits to the same branch. **Do not force-push once a reviewer has left feedback**, unless the user explicitly requests it.

### 8. Merge

Only on explicit user instruction.

```bash
gh pr merge <N> --squash --delete-branch
git checkout main && git pull --ff-only
git fetch --prune # Cleanup remote-tracking branches
```

## CI failures

Inspect logs with `gh run view --log-failed`.
