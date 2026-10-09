---
name: trust-test-authoring
description: Routes truST behaviour changes from the written specification to a native executable test. Use for any bug fix, feature, refactor, malformed-input handling, runtime-safety, VS Code, hardware, docs-only or supply-chain change, when deciding whether a specification or test is missing, and for PLC verification program work.
---

# truST test authoring

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

Logic is proven by a Rust test at the closest Rust boundary. A VS Code or Playwright test proves
only wiring and rendering, and never stands in for the Rust test of logic.

A behaviour is part of the product when two things agree:

```text
written specification -> native executable test
```

The written specification lives in `docs/specs/` (or in an IEC or product decision). The native
executable test asserts the behaviour at the closest practical boundary. It can be a Rust test, a
VS Code suite test, a Playwright test or a device-in-the-loop case.

Never derive product work from these: an invariant, a catalog link, a denominator row, an evidence
status, a proof level, a mutation result, a scanner fact, or a count of files or functions. They help
you find history. Planner, catalog, denominator, and evidence tooling is nonblocking maintenance and
cannot invent product requirements or tests. Only reading the owning specification and the native
assertions directly shows that a specification or test is missing.

## Route

1. Read the owning specification. If the behaviour is missing, ambiguous or contradictory there,
   write or update the specification first. When the choice belongs to the user, ask. Do not invent
   behaviour.
2. Read the current native assertions near the behaviour. A test name, catalog entry or report is
   not coverage.
3. Classify the behaviour as one of:
   - `already_covered`
   - `missing_spec`
   - `missing_test`
   - `behavior_defect`
   - `external_manual`
4. Write the assertion together with the change. Compile and run the focused test as you go
   (AGENTS.md, "Check as you go"). The full round runs once, at the end.
5. For a bug fix, when practical, record the focused test failing on its assertion before the fix,
   and the same test passing after. These are not that evidence: a compile, harness, dependency,
   registration or timeout failure. Never claim a run that did not happen.
6. For a refactor, keep behavior-lock tests that pass both before and after, with no change in
   behaviour.
7. Report:
   - the specification sections;
   - the test paths and names;
   - the commands and their results;
   - anything left unverified.

## By scenario

- **bug fix**: clarify the specification if needed, then write the regression assertion, make the
  fix, run the focused test, and finish with the final round.
- **refactor**: use behavior-lock tests. Split large test files by capability and keep test names
  stable where possible.
- **malformed input**: assert a stable rejection, and assert that nothing was partly applied.
- **VS Code**: register the test in `index.ts`. Prove visible behaviour in the rendered surface
  (`trust-vscode-quality`).
- **long end-to-end journey** (an example or rendered journey that runs for many minutes):
  - A check that does not block the next step records its failure, and the journey goes on. The
    journey fails at the end with every recorded failure, so one run names all the failures it
    reaches.
  - Before asking for another run after a failure, trace every remaining step through the code,
    including the steps the run never reached, and fix every problem found in one go.
  - When one user action has more than one code path, such as a restore in the same session and a
    restore from a saved checkpoint, the regression assertion covers the path the action really
    takes.
- **runtime safety**: run the runtime vertical (AGENTS.md). Fail-closed paths need their own
  assertions (`st-lsp-solid`).
- **hardware lab**: run the device-in-the-loop case on the real reviewed topology. If the hardware is
  unavailable, say so and leave the claim open (`trust-remote-builder`).
- **docs-only**: check every claim against native tests. Prose is not execution.
- **supply-chain**: run `bash scripts/supply_chain_gate.sh` and the release gates
  (`trust-ci-release-gates`).
- **CI-only regression**: this is a failure seen only in a platform compile or warning lane.
  - Keep the failed CI command and its log as the baseline.
  - Add a focused gate with the same target and flags.
  - After the fix, run the same cross-target command until it passes. Host-only tests do not close
    it.

## PLC verification program

The program's boards and history are in
`docs/internal/testing/checklists/plc-verification-program/`. Its corrective audit is
`phase18-zero-debt-execution-board.md` there. That audit may list only these items, each directly
confirmed:
- missing specifications;
- missing native tests;
- behaviour defects;
- explicit external or manual boundaries.

Pull-request workflows enforce native tests and specification drift checks. They may report
verification metadata, but they never reject a change only because a test lacks a catalog mapping.
Never derive product work from these metadata states: `reviewed_nonmapping`, ignored, skipped,
filtered out, unrun or zero-test.
