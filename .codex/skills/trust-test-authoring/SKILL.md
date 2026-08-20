---
name: trust-test-authoring
description: Use for every trust-platform behavior change, bug fix, refactor, malformed input, runtime safety, VS Code, hardware, docs, or supply-chain task that requires a written specification and a native executable test, including honest red-green or behavior-lock execution.
---

# truST Test Authoring

Use this skill before changing product behavior or native test ownership. The
detailed historical program documents live under
`docs/internal/testing/checklists/plc-verification-program/`.

## Product Contract

The substantive chain is:

```text
written specification -> native executable test
```

Never derive product work from an invariant, catalog link, denominator row,
evidence status, proof level, mutation result, scanner fact, file, or function.
Those may help locate history, but only direct inspection of the owning written
specification and native assertions can establish a missing specification or
test.

## Required Route

1. Read the owning product specification or IEC/product decision. If expected
   behavior is missing, ambiguous, or conflicting, update or obtain approval for
   that written specification before authoring a test.
2. Read current native test assertions at the closest practical boundary. Do
   not infer coverage from a test name, catalog entry, report, or broad suite.
3. Classify the behavior as `already_covered`, `missing_spec`, `missing_test`,
   `behavior_defect`, or `external_manual`.
4. For a bug fix or intentional behavior change, run the smallest native test
   before production edits and obtain the expected assertion failure. Compile,
   harness, dependency, timeout, and unrelated failures are not red evidence.
5. Implement only the minimum production change, then rerun the same focused
   test green. For a behavior-preserving refactor, establish a green
   behavior-lock first and keep it green; never manufacture a red.
6. For a missing test where current behavior already matches the specification,
   add the smallest native assertion and accept its honest green result. Do not
   change production merely to manufacture red evidence.
7. Use real hardware, browser rendering, or an external implementation when the
   written contract requires that boundary. Do not replace it with mocks or
   metadata.
8. Run focused checks continuously. Freeze one candidate, then run remote
   `fmt`, `clippy`, and `test-all` once at final validation.
9. Report exact specification sections, test paths/names, commands, and results.

## Scenario Routing

- **bug fix**: written specification, focused expected red, minimal fix, green.
- **refactor**: native behavior-lock before editing; no invented failure.
- **malformed input**: stable rejection and no-partial-apply assertion.
- **VS Code**: use `trust-vscode-quality` and prove rendered behavior when visible.
- **runtime safety**: use `st-lsp-solid` and run the required runtime vertical.
- **hardware lab**: use `trust-remote-builder`; keep unavailable hardware honest.
- **docs-only**: verify claims against native tests; prose alone is not execution.
- **supply-chain**: use `trust-ci-release-gates` and `trust-release-hygiene`.

Before push, run repository integrity checks that protect product specifications
and native tests. Planner, catalog, denominator, and evidence tooling is
nonblocking maintenance and cannot invent product requirements or tests.
