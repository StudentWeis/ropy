---
name: repo-coordination
description: Coordinate concurrent coding-agent sessions through task scopes, progress, and completion. REQUIRED before any repository change, including docs, configuration, and commits, and when scope/status changes or work ends. For read-only work or status requests, list active sessions. Advisory leases apply only to sessions sharing a ledger, not unshared filesystems.
---

# Repository coordination

Use `scripts/coord.py` as the only writer of coordination state. Resolve scripts relative to this skill; examples below assume the Git root. From a nested directory, use the skill's absolute path.

## Start and scope

Read-only tasks run `list` without registering. Write tasks register before any file creation, edit, deletion, or commit; register early when substantial investigation precedes the edit.

```bash
python3 .agents/skills/repo-coordination/scripts/coord.py list
python3 .agents/skills/repo-coordination/scripts/coord.py start \
  --mode write --task "Describe the current task" --scope path/to/file-or-directory
```

Retain the returned `session_id`. Scopes are repository-relative path prefixes; directories cover descendants. Declare only needed paths, without globs, absolute paths, or `..`. Reserve `.` for genuine whole-tree rewrites and explain why in the task description.

Before editing outside the scope, use `update`. Its `--scope` replaces the list, so repeat all paths still needed:

```bash
python3 .agents/skills/repo-coordination/scripts/coord.py update \
  --id SESSION_ID --scope original/path --scope newly-needed/path
```

## Resolve conflicts

`start` and `update` record the session and check conflicts. A `working` result may proceed; a `blocked` result must wait before editing overlapping paths. Run the following in the background and continue permitted work without asking the user to approve the wait:

```bash
python3 .agents/skills/repo-coordination/scripts/coord.py wait \
  --id SESSION_ID --timeout-minutes 480
```

`wait` polls conflicts, renews the lease, and returns `working` when blockers clear; then resume the edits. Read/write overlap warnings are informational.

`held_scopes` lists the paths currently reserved, which can differ from the requested scope. A blocked expansion retains earlier reservations, including child paths when requesting a parent. Continue writes only within `held_scopes`; narrow the request with `update` to release paths. Older working/waiting records without this field retain their declared scopes. Set `--status working` manually only after resolving the conflict another way, such as narrowing scope.

## Progress and completion

Update material changes to the task, phase, scope, status, or blocker; renew the lease at meaningful milestones during long operations:

```bash
python3 .agents/skills/repo-coordination/scripts/coord.py update \
  --id SESSION_ID --phase testing --note "Running targeted tests"
python3 .agents/skills/repo-coordination/scripts/coord.py heartbeat --id SESSION_ID
```

Use `--status waiting` when a required external result or approval is pending and the reservation must remain active. Optional feedback does not keep a completed task open. Archive registered tasks before reporting completion or abandonment; never mark unfinished work complete merely to release scope:

```bash
python3 .agents/skills/repo-coordination/scripts/coord.py finish \
  --id SESSION_ID --result completed --summary "What changed and what was verified"
```

Read-only tasks need no `finish`; later revision requests register a new task.

Every ledger command and status snapshot automatically archives expired leases under the lock. Use `coord.py cleanup` for explicit cleanup and archive details. Expired sessions cannot be revived by update, heartbeat, wait, or finish: inspect the working tree and register again before resuming.

## Recovery and status

On command failure or wait timeout, check the interpreter, platform, error, and readable ledger records; fix invocation problems or retry when recovery is plausible. Locking requires POSIX `flock` on macOS/Linux. Read-only work may continue after a failed query if the limitation is disclosed; an unlocked ledger read does not prove that no sessions are active. Writes still require registration. Never bypass locking or edit state JSON to recover.

Pause only overlapping paths during conflict recovery. Ask the user only if a remaining blocker requires a priority decision or external prerequisite; report blocked required work as incomplete.

Use `coord.py list` for scopes, session IDs, or machine-readable active records. For task titles and recent history:

```bash
python3 .agents/skills/repo-coordination/scripts/status.py
watch -c -n 2 python3 .agents/skills/repo-coordination/scripts/status.py --color
```

Other display and output options are documented by each command's `--help`.

## State boundaries

- A lease does not authorize overwriting user or other-agent changes, including unclaimed working-tree changes.
- Never mutate another session's record; expired tasks are handled only by automatic or explicit cleanup.
- Keep secrets, tokens, private prompts, and sensitive data out of task notes.
- Runtime state uses the self-ignored `.agent-coordination/` directory, outside the potentially read-only `.agents/` skill directory.
- Separate Git worktrees share no state by default; set `AGENT_COORDINATION_DIR` to one common absolute directory when they intentionally need a shared ledger.
