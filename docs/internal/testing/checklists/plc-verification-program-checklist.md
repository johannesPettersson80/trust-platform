# truST PLC Verification Program Checklist

Status: corrective closeout. Phase 16 completed the reviewed program on
2026-07-19. The later per-function Phase 17/18 expansion is being removed. The
active board now audits only surviving post-closure product behavior against
its written specification and native executable test.

Created: 2026-07-07. Split into focused documents: 2026-07-08.

## Start Here

The detailed verification program now lives under:

```text
docs/internal/testing/checklists/plc-verification-program/
```

Read in this order:

1. [phase18-zero-debt-execution-board.md](plc-verification-program/phase18-zero-debt-execution-board.md)
   (the only active post-closure specification-and-test sequence; current row
   `VERIF-P18-SPEC-TEST-005`)
2. [README.md](plc-verification-program/README.md)
   (current direct-contract orientation plus historical index)
3. [policy.md](plc-verification-program/policy.md),
   [metadata-model.md](plc-verification-program/metadata-model.md),
   [test-taxonomy.md](plc-verification-program/test-taxonomy.md),
   [verification-areas.md](plc-verification-program/verification-areas.md), and
   [implementation-board.md](plc-verification-program/implementation-board.md)
   (retired Phase 0-16 campaign records; none sequences current product work)
4. [fable-review-brief.md](plc-verification-program/fable-review-brief.md) and
   [execution-slice-001.md](plc-verification-program/execution-slice-001.md)
   (historical review and execution records only)

## Purpose

The goal is not more metadata. The substantive product contract is:

```text
written specification -> native executable test
```

Metadata may index that relationship for existing tooling, but a missing or
stale metadata link cannot create product work or override direct specification
and test evidence.

## Historical Implementation Order (completed 2026-07-19)

Items 1-11 below describe the accepted historical program. They do not
sequence current product work or create new specification/test requirements.

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
12. Post-closure behavior delta: preserve valid product fixes, specifications,
    native tests, and required fixtures; remove the per-function and proof-status
    campaign layers; resolve only directly confirmed missing specifications,
    missing native tests, or behavior defects; then run final remote gates once.

Scanner, denominator, invariant, catalog, evidence, and mutation state cannot
create product work. Only an observable behavior with a directly confirmed
missing written specification or native executable test is current work.

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
