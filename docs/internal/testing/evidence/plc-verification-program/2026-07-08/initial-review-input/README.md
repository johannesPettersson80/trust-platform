# truST PLC Verification Program

Status: draft for external review. Do not implement new runners, move tests, or
change gate semantics from this program until the review gate in
`implementation-board.md` is cleared.

Created: 2026-07-07. Split into focused documents: 2026-07-08.

## Purpose

The goal is not "more tests." The goal is that every important truST product
claim is traceable to:

1. a written specification or explicit spec gap,
2. an invariant,
3. a test or proof plan,
4. a suite/gate,
5. durable evidence,
6. a release/public-claim status.

This is a SQLite-style proof discipline adapted to PLC risk:

- Wrong output is worse than a crash.
- False healthy status is worse than an honest fault.
- Silent source/codegen corruption is worse than a refused build.
- Hardware behavior cannot be proven by mocks alone.
- IEC semantics need IEC/spec or documented-deviation oracles, not parity with
  another truST engine that may share the same bug.

## Document Map

- `policy.md`: stop gates, vocabulary, code-change discipline, spec-gap rules,
  test-refactor policy, suite tiers, metrics, and final definition of done.
- `metadata-model.md`: machine-readable record shapes, schema files, status
  vocabulary, authority precedence, schema versioning, and traceability reports.
- `test-taxonomy.md`: code-area matrix, test classes, coverage dimensions,
  malformed-input taxonomy, supply-chain/security tests, and platform tests.
- `verification-areas.md`: domain ownership, invariant classes, harnesses, and
  initial high-risk invariant seeds.
- `implementation-board.md`: staged implementation checklist.
- `fable-review-brief.md`: prompt and acceptance criteria for external review.
- `../plc-verification-program-checklist.md`: short entrypoint that points here.

## Implementation Order

The program must start with specifications, not tests.

1. Create the verification skeleton.
2. Inventory specification/oracle sources.
3. Inventory existing tests.
4. Classify spec gaps separately from test gaps.
5. Run the bytecode/VM pilot first.
6. Broaden to runtime safety, IEC/HIR, protocols, editor/UI, release, and
   hardware lab only after the pilot produces useful reports.

Most important safety area:

- `trust-runtime`, especially scan-cycle safety and the HIR -> bytecode -> VM
  seam.

First implementation pilot:

- `crates/trust-runtime/src/bytecode/**`
- `crates/trust-runtime/src/runtime/vm/**`
- `crates/trust-runtime-core/src/bytecode/**`
- `crates/trust-runtime-core/src/vm/**`
- `crates/trust-runtime-core/src/value/**`
- existing bytecode/VM tests and malformed bytecode fixtures

Why this pilot:

- safety-critical,
- deterministic,
- malformed-input-heavy,
- does not require hardware, browser, VS Code, or network proof,
- exercises spec-source inventory, invariant mapping, coverage gaps, ignored
  protective tests, and validator/report self-tests.

## Current Stop

Implementation is blocked until external review returns `clear` or
`clear-with-edits`, and required checklist edits are folded in.

Allowed before review:

- edit these planning documents,
- run static/doc consistency checks,
- prepare the Fable review input.

Not allowed before review:

- implement verification runners,
- move existing tests,
- mark coverage as validated,
- change runtime/compiler/LSP behavior from this board.

## Storage Summary

Executable tests stay where their native tooling expects them:

```text
crates/*/src/**              In-source unit tests.
crates/*/tests/**            Crate integration and regression tests.
editors/vscode/src/test/**   VS Code extension tests.
conformance/**               Public deterministic conformance tests.
fuzz/**                      Fuzz targets and minimized corpora.
scripts/**                   Gate runners, report renderers, validators.
```

Verification metadata lives under the future `verification/` control plane:

```text
verification/
  invariants/
  suites/
  schemas/
  test-catalog.toml
  ignored-tests.toml
  risk-register.toml
  evidence-index.toml
  spec-sources.toml
  spec-gaps.toml
```

Generated reports are artifacts under `target/gate-artifacts/**` or CI
workspace artifact paths unless a reviewed summary is intentionally committed.
