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

### 1. Open or reuse the Issue

If the authorized contribution already has an Issue, record its number as `<N>` and reuse it; delegated workers must not create duplicate Issues. This is not an Issue skip. Otherwise, create one as follows.

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

### 2. Choose the coordinated workspace and branch

Inspect Git status before changing branches or writing files. When the agent environment requires `coord` or the task already uses it, follow that skill first: inspect active sessions, register the intended write scopes, and verify the returned worktree, branch and `held_scopes`. A path lease does not grant exclusive access to the shared Git index or permission to change another session's branch. Contributors without `coord` use the ordinary branch flow below; this does not override an agent environment that requires coordination.

For a new, unoccupied workspace with no unrelated changes, branch from fresh `main`:

```bash
git checkout main && git pull --ff-only
git checkout -b <type>/<kebab-slug> # e.g. feat/grid-layout, fix/clipboard-empty-x11
```

**Coordinated workspace exception:** when the authorized task already has an Issue and a contribution branch, reuse both in the worktree returned by `coord`; do not run the checkout/pull commands above. An independent worktree created by `coord` uses its returned branch and recorded integration target instead of switching to `main`. Follow `coord` for integration and post-integration checks. Neither exception skips the Issue, precheck, Conventional Commits, PR or explicit merge authorization requirements. Record the existing Issue and intended PR owner in the handoff.

In a shared worktree, designate one owner for staging, hooks, commits, pushes and the PR. Other workers edit only their held paths and report their checks without touching the index or switching branches. If branch ownership or staged inputs are unclear, pause Git mutations and coordinate; do not stash, reset, clean or move another participant's changes to obtain a clean state.

### 3. Implement

Follow the project's development guidelines.

### 4. Precheck

Must pass before any commit:

```bash
./scripts/precheck.sh
```

### 5. Commit

The designated Git owner stages only the files for the ready change, including new files and deletions. Inspect the staged diff and status before committing; leave unrelated changes unstaged. In a shared worktree, confirm that all included workers have finished their held paths and that the precheck covers the combined commit inputs. A worker's targeted tests do not replace Step 4.

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

Only on explicit user instruction. Check Git status and, when coordination is in use, active `coord` sessions again before merging or cleaning up. Confirm that no worker still depends on the PR branch or has unfinished changes for that PR. If it is still in use, defer merge and cleanup until the owners finish; do not abandon their work implicitly.

Once the PR is ready and merging is authorized, merge without coupling it to local branch deletion or a checkout:

```bash
gh pr merge <N> --squash
```

Local cleanup is a separate operation. Run checkout/pull only in a clean worktree with no active participants relying on its current branch or HEAD and no unfinished Git operation:

```bash
git checkout main && git pull --ff-only
git fetch --prune # Cleanup stale remote-tracking branches
```

Delete the contribution branch locally or remotely only after confirming that no active session or linked worktree still uses it and that all required work is preserved in the merged PR or integrated commit. Use the `coord` recovery/workspace cleanup procedure for an independent worktree; do not delete its branch or directory manually while it is active. If checkout, pull or cleanup fails, retain the branch/worktree and report the failed step; do not force deletion or reset. A successful remote merge does not imply successful local cleanup.

## CI failures

Inspect logs with `gh run view --log-failed`.
