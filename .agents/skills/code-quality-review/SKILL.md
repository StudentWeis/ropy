---
name: code-quality-review
description: Review a code change, diff, pull request, module, or test suite for code quality, comment and documentation quality, and test quality. Use when asked to review code, audit maintainability, inspect comments, assess test coverage or test usefulness, perform a pre-merge quality check, or produce prioritized quality findings. Support both repository-wide and language-agnostic reviews; remain read-only unless the user explicitly asks for fixes.
---

# Code Quality Review

Perform an evidence-based review across three dimensions: production code, comments and documentation, and tests. Prioritize defects and maintainability risks that can change behavior or make future changes unsafe. Do not turn personal style preferences into findings.

## Establish the Review Contract

1. Determine the target from the user's request: explicit files, working-tree changes, a commit range, a branch, a pull request, or the whole repository.
2. If the target is ambiguous, choose the narrowest reasonable scope available from the current context and state the assumption. Ask only when different scopes would materially change the result.
3. Read repository instructions and the testing guide before judging the change. Treat local conventions and documented requirements as authoritative.
4. Infer the intended behavior from the issue, request, surrounding code, public API, and tests. Separate confirmed requirements from assumptions.
5. Keep review and implementation separate. Inspect and report by default; modify files only when the user explicitly requests fixes.

For a diff review, inspect both the diff and enough surrounding code to understand callers, invariants, error paths, and existing tests. Do not review changed lines in isolation.

## Build an Evidence Base

- Enumerate changed files and classify production code, tests, generated files, configuration, and documentation.
- Search for call sites, analogous implementations, shared helpers, and tests that exercise the affected behavior.
- Run the smallest relevant formatter, compiler, linter, and test commands that the repository supports. Expand only when risk or failures justify it.
- Treat command success as supporting evidence, not proof of quality. Review behavior and assertions directly.
- Distinguish introduced problems from pre-existing problems. Report pre-existing issues only when the change makes them newly reachable, more severe, or directly relevant to the request.
- Record uncertainty. Do not present an unverified suspicion as a finding.

## Review Code Quality

Check correctness before aesthetics.

### Behavior and safety

- Trace normal, boundary, error, cancellation, retry, and cleanup paths.
- Check validation at trust boundaries and assumptions about ordering, uniqueness, nullability, encoding, time, concurrency, and platform behavior.
- Check whether errors are preserved, classified, handled at the right layer, and observable without exposing sensitive data.
- Check resource ownership, lifecycle, atomicity, idempotency, and partial-failure behavior where relevant.
- Check security and privacy implications when input, permissions, secrets, serialization, filesystem access, or external commands are involved.

### Design and maintainability

- Prefer the simplest design that preserves required behavior.
- Flag duplication only when it creates a realistic divergence or maintenance risk; do not demand abstraction for superficial similarity.
- Check names, module boundaries, dependency direction, cohesion, coupling, and API contracts.
- Identify hidden side effects, surprising control flow, boolean blindness, temporal coupling, and invalid states that the design permits unnecessarily.
- Check consistency with established repository patterns unless those patterns conflict with an explicit requirement.
- Treat formatter and linter output as authoritative for mechanical style. Avoid repeating automated diagnostics unless they explain a larger problem.

## Review Comment and Documentation Quality

Evaluate comments for truthfulness and decision value, not quantity.

- Verify that comments and API documentation match current behavior, parameters, errors, side effects, units, ownership, and concurrency guarantees.
- Prefer explanations of intent, constraints, invariants, tradeoffs, or non-obvious reasons. Flag comments that merely narrate clear syntax when they add noise or can become stale.
- Require documentation for public contracts and safety-critical or surprising behavior when repository conventions call for it.
- Check that workarounds explain the external constraint and, when appropriate, include a traceable issue or removal condition.
- Check TODO, FIXME, and SAFETY comments for specificity, ownership context, and validity.
- Flag commented-out code and obsolete explanations when version control is the better record.
- Do not demand comments to compensate for confusing code when a small code improvement would express the idea more reliably.

## Review Test Quality

Map tests to behavioral risks before considering line coverage.

### Behavioral coverage

- Identify the change's important behaviors and failure modes, then map each to an existing or missing test.
- Check happy paths, boundaries, invalid inputs, error propagation, state transitions, regressions, and platform-specific behavior in proportion to risk.
- Require a regression test for a bug fix when the failure can be reproduced deterministically at a sensible test layer.
- Do not demand tests for declarations or trivial forwarding that cannot fail meaningfully; test the behavior at the layer where a defect would be observable.

### Assertion strength

- Verify that each test could fail for the defect it claims to catch.
- Prefer assertions on externally meaningful outputs, state, events, or errors over incidental implementation details.
- Flag tests that only prove setup completed, duplicate the implementation, assert tautologies, or would pass after removing the behavior under test.
- Check that parameterized cases are meaningfully distinct and that snapshots or golden files are narrowly scoped and reviewed.

### Reliability and maintainability

- Check isolation from execution order, shared mutable state, wall-clock timing, random seeds, network availability, locale, and machine-specific paths.
- Prefer deterministic fakes at external boundaries. Flag excessive mocking when it only verifies call choreography and misses real behavior.
- Check cleanup, temporary resource handling, concurrency synchronization, and retry or timeout behavior.
- Check test names and structure against repository conventions. Keep fixtures readable and focused on behavior.
- Treat coverage percentages as a discovery aid, never as a substitute for evaluating risk and assertion quality.

## Validate Each Finding

Report a finding only when all of these are true:

1. A specific behavior, contract, repository rule, or maintainability property is violated.
2. The evidence identifies an exact location and a realistic triggering scenario.
3. The impact is material enough that the author would likely act on it.
4. The recommendation addresses the cause without requiring an unjustified redesign.
5. The finding is not a duplicate or merely the downstream symptom of a stronger root-cause finding.

Use these severities:

- `P0` — release-blocking or catastrophic: data loss, severe security exposure, or broadly unusable behavior.
- `P1` — high: likely correctness, security, or reliability failure in normal use.
- `P2` — medium: real defect or maintainability/test weakness with bounded impact.
- `P3` — low: worthwhile improvement with limited immediate risk. Use sparingly; omit pure polish.

Calibrate severity from likelihood, blast radius, detectability, and reversibility. Never inflate severity to make a review look thorough.

## Report the Review

Lead with findings ordered by severity, then source location. For each finding include:

```text
[P2] Concise, actionable title — path/to/file:line
Evidence: What the code or test does and the triggering scenario.
Impact: The concrete failure or maintenance cost.
Recommendation: The smallest robust direction for correction.
```

Use exact file and line references whenever available. Keep code excerpts minimal.

After findings, include:

- assumptions or unresolved questions that materially affect the verdict;
- a short three-axis summary for code, comments, and tests;
- commands run and any validation limits.

If there are no actionable findings, say so explicitly and still note validation performed and residual risks. Do not invent findings to fill every category. Match the report language to the user's language unless requested otherwise.
