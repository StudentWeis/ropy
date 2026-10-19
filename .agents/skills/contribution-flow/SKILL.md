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

Pick the appropriate issue template from `.github/ISSUE_TEMPLATE/` — using a template is **mandatory**. Allocate a private body file for this task:

```bash
issue_body=$(mktemp "${TMPDIR:-/tmp}/ropy-issue.XXXXXX")
```

Fill the template into the returned file, then use that same path:

```bash
gh issue create --title "<type>: ..." --label "<appropriate label>" --body-file "$issue_body"
```

Capture the returned number as `<N>`. After successful creation, remove only this task's body file. If creation fails, retain the file for retry. When commands run in separate shells, carry the returned absolute path forward explicitly.

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

Stage only the files for this change, including new files and deletions. Inspect
the staged diff and status before committing; leave unrelated changes unstaged.

```bash
git add -- <paths-for-this-change>
git diff --cached
git status --short
git commit -m "<type>(<scope>): <short description>

Refs #<N>"
```

Multiple commits are fine — they are squashed on merge.

### 6. Push & Open the PR

Use `.github/PULL_REQUEST_TEMPLATE.md` — using the template is **mandatory**. Allocate another private body file:

```bash
pr_body=$(mktemp "${TMPDIR:-/tmp}/ropy-pr.XXXXXX")
```

Fill the template into the returned file, then:

```bash
git push -u origin HEAD
gh pr create --base main --title "<type>(<scope>): ..." --body-file "$pr_body"
```

Report the PR URL back to the user. After successful creation, remove only this task's PR body file; retain it on failure and carry its absolute path across separate shells as needed.

### 7. Iterate on review

Push additional commits to the same branch. **Do not force-push once a reviewer has left feedback**, unless the user explicitly requests it.

### 8. Merge

Only on explicit user instruction.

```bash
gh pr merge <N> --squash --delete-branch && \
  git checkout main && git pull --ff-only && \
  git fetch --prune # Cleanup stale remote-tracking branches
```

## CI failures

Inspect logs with `gh run view --log-failed`.
