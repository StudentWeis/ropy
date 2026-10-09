---
name: repo-health
description: Inspect repository maintainability, robustness, and simplicity without applying fixes. Use for repository health audits, 仓库体检, and broad reviews of checks, documentation, architecture, failure behavior, or unused content. Report evidence-based findings under the repository's own conventions.
---

# Repo Health

Inspect and report. Do not edit source, configuration, or documentation, stage changes, or apply fixes during the audit. Normal local build/test artifacts are acceptable. If fixes are also authorized, finish the inspection before starting a separate fix phase. Do not create proposal files or inline TODOs during the audit.

## Judgment criteria

Judge against the repository's actual purpose, supported environments, and failure consequences:

- **Necessity:** establish requirements and invariants; do not impose unneeded public-project infrastructure or compatibility promises.
- **Simplicity:** reduce concepts, states, and dependencies while preserving required failure guarantees.
- **Clarity:** favor understandable contracts, control flow, and side effects that can be changed locally.
- **Ownership:** keep cohesive responsibilities together and shared rules in one authoritative place.

File size, names such as `utils`, stylistic preferences, TODO/FIXME/HACK markers, explanatory WHAT comments, and a missing README are review leads, not defect evidence. Recommend the smallest change that solves a concrete problem; explain maintenance cost and material tradeoffs. Moving complexity into a wrapper or adding a dependency is not inherently simpler.

## Inspection workflow

### 1. Establish scope

- Resolve the Git root; read applicable `AGENTS.md`, coordination rules, and actual build/check entrypoints. Record the initial working-tree status and preserve existing changes.
- Default to repository-wide inspection unless narrowed by the user. Inventory the scope with `git ls-files` and relevant nonignored untracked files, then focus manual review on changed areas, complex modules, and boundaries. Distinguish tracked content from ignored runtime output; tracked files remain in scope even if an ignore pattern matches them.
- Treat `--base <ref>` as an instruction argument, not a skill executable. Validate the ref, then prioritize its diff against the working tree without silently narrowing scope. Report invalid refs. Label a finding new or pre-existing only after checking the base content; otherwise label its origin unknown. Claim a quality-gate comparison between two versions only after checking both versions.

### 2. Run existing checks

Read recipes and scripts for side effects before using the maintained aggregate check command. Use non-mutating format/lint/test/build checks; do not run auto-fix hooks, installations, deployments, or commands that apply machine settings. Do not install missing dependencies or create a parallel check framework.

If an aggregate stops early, run remaining independent checks through existing entrypoints where practical. If none exists, select checks from project configuration. Avoid repeating successful expensive checks without a reason.

Record each command as **PASS** (successful), **FAIL** (demonstrated quality failure, with a short excerpt), **BLOCKED** (environment/dependency prevented a usable result), or **NOT RUN** (with a reason). Continue static review when blocked. Recheck working-tree status after checks and reproductions; report unexpected changes without reverting them.

### 3. Review responsibilities

Review gaps beyond automated coverage:

#### Documentation and contracts

Compare docs, skills, examples, commands, and paths with behavior. Find stale or duplicated explanations, abandoned commented-out code, and missing explanations for subtle invariants.

#### Structure and ownership

Trace dependency direction, public surfaces, configuration ownership, shared operations, entrypoints, and boundaries between persistent and runtime data. If responsibilities are mixed or layers impede navigation or local changes, explain the impact and name a concrete destination for consolidation.

#### Simplification

Look for mirrored facts, speculative extension points, and overlapping lifecycle mechanisms. For each copy, validator, retry, or fallback, identify the input origin, owner, and failure guarantee. Remove protection only when its condition is unsupported or already enforced elsewhere.

Distinguish trusted internal calls from parsing, persistence, queue, process, and network boundaries. In asynchronous code, identify who owns state, readiness, cancellation, and cleanup, and how each changes. Preserve the separate guarantees for rollback, terminal outcomes, and teardown.

Before recommending a builtin or maintained dependency, compare it with the implementation, tests, and docs it would replace. Check runtime compatibility, semantics it does not cover, dependency footprint, and glue code that would remain.

#### Behavior and robustness

Trace missing or invalid input, dependency failures, interruption, partial writes, error propagation, cleanup, retries, and concurrency where relevant. Identify the invariant or user data at risk. Check whether failure can appear to succeed.

Use code paths or safe local reproductions. Never inject failures into real user configuration or external systems. Recommend focused regression tests for demonstrated risks, not coverage targets or tests that mirror the implementation.

#### Files and retained content

Investigate logs, caches, backups, obsolete scaffolding, orphaned assets, duplicates, and disabled blocks. Before calling content unused, check production callers, manual CLI entrypoints, application/skill discovery, symlinks, manifests, build scripts, dynamic imports, and documented external consumers.

For shims, design docs, and removed features, compare the historical rationale with surviving formats, migrations, compatibility paths, and inbound links. Respect archive rules and keep current decisions at their owner. Personal settings, lockfiles, fixtures, snapshots, vendored/generated assets, and explicit defaults can have intentional roles. Tests or text searches alone do not establish that content is unused.

## Evidence and report

Separate confirmed defects from design choices and unverified questions. Each finding needs a clickable `path:line` reference (or file link), concrete evidence or trigger, practical impact, and a suggested fix with material tradeoffs. Use **high** for demonstrated breakage or consequential boundary violations, **medium** for meaningful correctness/maintenance problems or misleading docs, and **low** for justified minor cleanup. Include origin evidence when reviewing against `--base`.

Produce a concise Markdown report in the user's language:

1. Verdict, scope, relevant uncommitted changes, and confirmed counts by severity.
2. Quality-gate table with actual commands, statuses, and failure/blocking reasons.
3. Findings grouped by the responsibilities above, counting each root cause once and omitting empty groups.
4. Separate design opportunities, unverified questions, and coverage limits, including sampled or unreviewed areas. For opportunities, state the cost, proposed consolidation, capability or guarantee given up, and observable validation needed.
5. At most ten prioritized actions, referencing findings without repeating them.

A clean report is valid. Passing automated checks do not establish complete manual coverage or prove that a proposed simplification preserves behavior.
