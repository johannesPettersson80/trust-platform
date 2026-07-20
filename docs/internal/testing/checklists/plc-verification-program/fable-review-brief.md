# Fable Review Brief

Status: completed 2026-07-08. Verdict `clear-with-edits`; findings V-01..V-15,
fold verification, and recorded decisions live under
`docs/internal/testing/evidence/plc-verification-program/2026-07-08/`.

This is a good use for Fable because the expensive reasoning should happen
before implementation. The review should attack the verification-program plan,
not generate code.

Give Fable this prompt together with the full document folder:

```text
You are reviewing a proposed verification program for truST, a PLC
compiler/runtime/editor system. Treat wrong PLC outputs, false healthy status,
silent source transformation, malformed bytecode execution, and unproven
hardware behavior as safety-critical.

Review this document set as a plan before implementation:

- README.md
- policy.md
- metadata-model.md
- metadata-evidence-traceability.md
- test-taxonomy.md
- verification-areas.md
- implementation-board.md
- fable-review-brief.md

Do not write code. Review the plan.

Core questions:

1. Is the document split correct, or should responsibilities be split
   differently before implementation?
2. Are the stop gates strong enough to prevent accidental overclaiming?
3. Is "specification source inventory before test inventory" the right order?
4. Does the spec-source model capture authority, freshness, public/internal
   visibility, external standards, reviewed decisions/deviations, stale docs,
   and missing specs well enough?
5. Are the authority precedence rules correct when IEC/protocol standards,
   public docs, truST design contracts, reviewed decisions, and reviewed
   deviations disagree?
6. Does the metadata model capture enough information to connect spec sources,
   spec gaps, invariants, tests, suites, gates, evidence, risks, and public
   claims?
7. Are the record shapes complete for invariant, spec source, spec gap, test
   catalog, suite, ignored test, and risk register?
8. Are schema versioning and migration rules strong enough to prevent stale
   metadata from silently passing?
9. Are the machine states consistent and sufficient? Check gap_open, spec_gap,
   blocked, deferred, rejected, unproven, and validated.
10. Does the suite taxonomy cover PLC-grade proof: normal execution, malformed
    input, runtime faults, restart/retain, communication failure, hardware
    behavior, source/editor corruption, UI truth, security/supply chain,
    platform/package behavior, and release evidence?
11. Are the coverage dimensions strong enough to prevent one-test false
    confidence? Which dimensions are missing?
12. Is the malformed-input taxonomy complete enough for ST source, HIR, PLCopen
    XML, bytecode, config, protocol payloads, LSP/editor protocol, API/HMI
    payloads, persisted state, supply chain, and platform/filesystem variation?
13. Are the proposed suite tiers right: veryquick, pr, nightly, release,
    hardware_lab?
14. Is the code-area matrix precise enough to tell future agents which tests are
    required for a touched path?
15. Is the test-first code-change discipline precise enough for bug fixes, new
    features, safety fixes, refactors, docs-only changes, hardware-only claims,
    and release/security claims?
16. Is the existing-test refactor policy strong enough to improve the suite
    without losing coverage?
17. Are the verification-tooling self-tests sufficient? What known-bad fixtures
    are missing?
18. Is the bidirectional traceability report sufficient:
    spec source -> invariant -> test -> suite/gate -> evidence -> public claim,
    and reverse?
19. Are any required verification areas missing?
20. Which existing truST checklists, gates, or docs must this program integrate
    with before implementation?
21. What would SQLite-style rigor require here that the plan still misses?
22. Which parts should be implemented first for maximum PLC safety value?
23. What should be rejected as too broad, too costly, or likely to become stale?
24. Is the Codex skill/agent sync phase sufficient to make the workflow durable
    after implementation without teaching future agents a stale draft process?

For every finding, provide:

- severity,
- exact document and row or missing row,
- why it matters for PLC safety or release truth,
- proposed checklist edit,
- whether implementation should remain blocked until fixed.

Do not accept coverage percentage, line count, or test count as safety proof.
Demand independent oracles, spec-gap handling, and failure-mode tests for
safety-critical claims.
```

## Review Acceptance

The checklist rows for review acceptance live in `implementation-board.md`:

- `VERIF-REVIEW-001`: Fable review returns `clear`, `clear-with-edits`, or
  `blocked`.
- `VERIF-REVIEW-002`: Every required edit is folded into the document set.
- `VERIF-REVIEW-003`: Disputed recommendations are recorded with decision and
  owner.
- `VERIF-REVIEW-004`: Only after review is folded in may Phase 1 implementation
  start.
