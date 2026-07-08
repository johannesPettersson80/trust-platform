# truST PLC Verification Program Checklist

Status: draft for external model review. Do not implement new runners, move
tests, or change gate semantics until the review gate is cleared.

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
7. [fable-review-brief.md](plc-verification-program/fable-review-brief.md)

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

Implementation is blocked until
[VERIF-REVIEW-004](plc-verification-program/implementation-board.md) is cleared.

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
