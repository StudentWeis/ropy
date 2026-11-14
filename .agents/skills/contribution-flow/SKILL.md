---
name: contribution-flow
description: Repository contribution workflow for changes intended for commit. Covers Issue → branch → checks → commit → PR → merge → cleanup, including coordinated workspaces and authorization boundaries.
---

# Contribution flow

Run commands from the repository root. Follow `AGENTS.md` for development rules and `coord` for workspace coordination. Stop at the user's requested stage; implementing a change does not by itself authorize committing, pushing or merging.

## Conventions

- **Type**: `feat | fix | docs | style | refactor | perf | test | build | ci | chore | revert`. Keep it consistent across the Issue title, branch prefix, commits and PR title.
- **Scope**: infer from the affected module, such as `core`, `gui` or `clipboard`. Required on commits and the PR title, optional on the Issue title, omitted from the branch name.
- **Subject**: lowercase, imperative mood, no trailing period.
- **Body files**: allocate a separate `mktemp` file for each Issue or PR. Carry its absolute path across shells. Delete only that file after successful creation; retain it on failure for retry.

## 1. Open or reuse the Issue

Reuse the task's existing Issue, including for delegated work. Otherwise, fill the appropriate `.github/ISSUE_TEMPLATE/` template into a private body file; templates are mandatory.

```bash
issue_body=$(mktemp "${TMPDIR:-/tmp}/ropy-issue.XXXXXX")
# Fill the body from the selected template before creating the Issue.
gh issue create --title "<type>: <subject>" --label "<label>" --body-file "$issue_body"
```

Record the Issue number as `<issue>`. Skip the Issue only at the user's explicit request, and record the skip in the PR description.

## 2. Choose the workspace and branch

Inspect Git status and active sessions before writing or switching branches. When coordination is required, follow `coord`: register scopes, verify `held_scopes`, and use the returned worktree and branch. Reuse an existing task Issue and contribution branch. An independent worktree uses its recorded integration target; do not switch it to `main`. These workspace exceptions do not waive the remaining contribution steps.

For an ordinary clean, unoccupied workspace, start from fresh `main`:

```bash
git checkout main && git pull --ff-only &&
    git checkout -b <type>/<kebab-slug>
```

In a shared worktree, designate one owner for staging, hooks, commits, pushes and the PR; record that owner and the Issue in handoffs. Other workers edit only their held paths. If branch ownership or staged inputs are unclear, pause Git mutations and coordinate. Do not stash, reset, clean or move others' changes to obtain a clean state.

## 3. Implement and run precheck

Follow the development rules and [Testing](../../../docs/testing.md). Before every commit, pass the full gate on the intended commit inputs:

```bash
./scripts/precheck.sh
```

Use `--check` when checks must not modify files. Targeted tests or the lightweight phase alone do not replace the full gate. In shared worktrees, included workers must finish their paths before combined verification and staging.

## 4. Commit

The Git owner stages only ready task files, including additions and deletions. Review the staged diff and status; leave unrelated changes unstaged.

```bash
git add -- <task-paths>
git diff --cached
git status --short
git commit -m "<type>(<scope>): <subject>

Refs #<issue>"
```

Omit `Refs` if the Issue was explicitly skipped. Multiple commits are allowed; merge uses squash.

## 5. Push and open the PR

Fill `.github/PULL_REQUEST_TEMPLATE.md` into a new private body file. The template is mandatory.

```bash
pr_body=$(mktemp "${TMPDIR:-/tmp}/ropy-pr.XXXXXX")
# Fill the PR body before creating the PR.
git push -u origin HEAD &&
    gh pr create --base main --title "<type>(<scope>): <subject>" --body-file "$pr_body"
```

Report the PR URL and record its number as `<pr>`.

## 6. Review and CI

Push review fixes to the same branch. After reviewer feedback, do not force-push without explicit user authorization.

Wait for complete CI on the latest PR head, including the aggregate gate and required checks. A partial list of passing jobs is not a CI pass. Inspect failures with `gh run view <run-id> --log-failed`; do not bypass failed, pending or skipped required checks.

## 7. Merge when authorized

Merge only on explicit user instruction; “merge when CI passes” authorizes waiting and then merging. Recheck Git status, active sessions, the PR head and CI before merging. If another worker still depends on the branch or has unfinished PR changes, defer merge and cleanup without abandoning their work.

Record the contribution branch as `<branch>` and the checked head as `<head-sha>`, then merge without coupling it to checkout or branch deletion:

```bash
gh pr merge <pr> --squash --match-head-commit <head-sha>
```

Confirm the PR is merged and record the merge commit. Then continue to cleanup.

## 8. Clean up and report

After an authorized merge, finish local cleanup by default without a second confirmation, unless the user asked to retain the branch. Separate commands are a safety boundary, not permission to omit this step.

Recheck Git status and active sessions before cleanup. Require a clean worktree, no unfinished Git operation, no session or linked worktree depending on the contribution branch or this worktree's current HEAD, and all contribution work preserved in the merged result. For independent worktrees, use the `coord` recovery/workspace cleanup procedure instead of manually deleting their branch or directory.

For an ordinary worktree that meets these conditions:

```bash
git checkout main &&
    git pull --ff-only &&
    git branch -d <branch> &&
    git fetch --prune
```

If a prerequisite is unmet or a command fails, stop cleanup, retain the remaining branch/worktree and report the specific blocker. Do not force deletion or reset to hide the failure. Apply the same preservation and usage checks before any remote branch deletion.

Report the PR URL, checks, merge commit, current local branch and cleanup outcome. Distinguish “merged” from “cleanup complete”; name any retained branch/worktree and why it remains.
