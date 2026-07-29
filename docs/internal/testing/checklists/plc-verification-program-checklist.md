# truST PLC Verification Program Checklist

Status: active. The original program contract returned `clear-with-edits`
(2026-07-08), and those edits are folded. Phase 17 added the exhaustive
production-code denominator. Phase 18 was added on 2026-07-21 after the frozen
denominators proved that explicit `registered_backlog` and
`reviewed_nonmapping` populations were still unresolved semantic/test debt.
The program is not complete until the Phase 18 zero-debt rows and stop gates
close through independent acceptance.

Created: 2026-07-07. Split into focused documents: 2026-07-08.

## Start Here

The detailed verification program now lives under:

```text
docs/internal/testing/checklists/plc-verification-program/
```

Read in this order:

1. [README.md](plc-verification-program/README.md)
2. [policy.md](plc-verification-program/policy.md)
3. [metadata-model.md](plc-verification-program/metadata-model.md)
4. [test-taxonomy.md](plc-verification-program/test-taxonomy.md)
5. [verification-areas.md](plc-verification-program/verification-areas.md)
6. [implementation-board.md](plc-verification-program/implementation-board.md)
7. [phase18-zero-debt-execution-board.md](plc-verification-program/phase18-zero-debt-execution-board.md)
   (active zero-debt completion board; its pointer is `VERIF-P18-003`, current
   repository-wide authority/mapping campaign)
8. [fable-review-brief.md](plc-verification-program/fable-review-brief.md)
9. [execution-slice-001.md](plc-verification-program/execution-slice-001.md)
   (queued: Phase 16 readiness plus the first spec-to-green vertical; product
   edits remain blocked until the readiness rows close)

## Purpose

The goal is not more tests. The goal is proof discipline:

```text
spec source or spec gap
  -> invariant
  -> test/proof
  -> suite/gate
  -> evidence
  -> release/public-claim status
```

This is a SQLite-style verification program adapted to PLC risk. Wrong output,
false healthy status, silent source/codegen corruption, malformed bytecode
execution, and unproven hardware claims are treated as high-risk.

## Current Implementation Order

1. Review and baseline freeze.
2. Verification control-plane skeleton.
3. Specification source inventory.
4. Existing test catalog.
5. Bytecode/VM seam pilot.
6. Runtime safety.
7. HIR/IEC conformance.
8. Protocol and hardware lab proof.
9. Editor/LSP/VS Code source-safety proof.
10. HMI/UI/release/security/platform proof.

11. Execution: run the program and close every gap (board Phase 16; starts
    after Phase 10 and may run before or interleaved with Phases 11-15).
    First close the execution-readiness rows without changing product
    behavior. Then write missing specs, add and run missing tests, fix each
    proven failure through red/green proof, promote invariants only as far as
    evidence supports, close ledgers, and finally flip CI enforcement. The
    first product vertical is detailed in
    [execution-slice-001.md](plc-verification-program/execution-slice-001.md)
    (board row `VERIF-P16-001`).
12. Production-code denominator: inventory every behavior-bearing Rust,
    TypeScript/JavaScript, Python, shell, and verification-tooling fact; reject
    silent omissions and stale mappings; keep new/changed code fail-closed
    (board Phase 17).
13. Zero-debt semantic and test completion: replace every historical
    `registered_backlog`/open-gap fact with an authoritative specification and
    executable invariant/test chain, eliminate every `reviewed_nonmapping`
    test, and prove assertion strength, coverage, mutation, fuzz/property,
    concurrency/time, unsafe, real hardware/endurance, rendered UX, and
    supported-platform behavior (board Phase 18).

Final completion means zero known or hidden debt, not merely zero unclassified
rows. A code fact mapped to a backlog gap is still unspecified. A discovered
test assigned `reviewed_nonmapping` still has no accepted proof claim. Passing
broad suites or structural scanners cannot waive either population.

The final proof chain is:

```text
specified behavior -> invariant -> code -> executed test -> suite/evidence
  -> adequacy/environment proof -> frozen candidate -> GitHub/public proof
  -> independent final verdict
```

An explicit specification-gap route is interim blocking evidence, never a
final proof route.

The review gate
[VERIF-REVIEW-004](plc-verification-program/implementation-board.md) was
cleared on 2026-07-08; verdict and fold verification live under
`docs/internal/testing/evidence/plc-verification-program/2026-07-08/`.

## Why This File Is Short

The original checklist had grown past 2,000 lines and mixed policy, schemas,
taxonomies, phase rows, seed invariants, and review prompts. It was split to
avoid creating another god-file before implementation starts.

The split fixes these known issues:

- schema inventory includes every planned metadata file,
- all metadata records have required shapes,
- machine status vocabulary is normalized,
- spec authority precedence is explicit,
- spec inventory comes before test-to-claim mapping,
- durable evidence has a record type and cannot point at ignored local paths,
- bidirectional traceability reports are required,
- verification tooling self-tests are required,
- security/supply-chain and platform/package proof are first-class areas,
- the bytecode/VM pilot includes `trust-runtime-core` and the debug/force path,
- Fable review prompt covers the full model.
