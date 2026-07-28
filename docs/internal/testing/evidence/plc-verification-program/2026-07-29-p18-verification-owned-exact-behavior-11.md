# Phase 18 verification-owned exact-behavior campaign

Date: 2026-07-29

## Scope and result

This coherent campaign reviewed the complete remaining verification-owned
behavior group:

- six open behaviors in `VERIFICATION_CASE_ARTIFACT_HELPER_001`;
- five open behaviors in `VERIFICATION_IEC_TABLE_INVENTORY_001`;
- twelve exact demand-bearing tests;
- three already-mapped case-helper companions whose accepted assertion
  provenance changed with the source;
- fifteen total exact tests and seventeen behavior links.

The accepted transaction promotes twelve tests, refreshes three existing
proposal/decision/catalog bindings, and closes all eleven behavior demands.
The frozen equations move from 4,399 live denominator facts, 3,921 mapped, 478
reviewed nonmapping, and 38 missing behaviors to:

```text
4,401 live = 3,933 catalog_mapped + 468 reviewed_nonmapping
0 missing specification authority
27 confirmed missing behaviors
```

The two new IEC negative tests account for the live-denominator increase.
This is large-batch progress across `P18-003` through `P18-006`; none of those
rows closes globally.

## Exact checkouts and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Local branch: `plc-verification-program`
- Exact accepted campaign and assertion source commit:
  `1338cd7a48e690c43b65c7239216f651a97178b3`
- Accepted metadata transaction commit:
  `a124d727921776884fce10e78a62e00a12a365b2`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-verification-owned-11`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the canonical checkout and verified byte-for-byte before
the source campaign and again before final focused validation. Both accepted
revisions were clean.

## Test-first oracle closure

The generated-input proof boundary began with a focused expected red:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p xtask \
  generated_inputs_flow_through_helper_parameters_and_struct_fields \
  -- --nocapture
```

The test failed by its intended assertion because both dynamic reads remained
visible:

```text
digest::std::fs::read:unresolved-oracle
run_case::std::fs::read:unresolved-oracle
```

The assertion extractor now propagates only closed generated-path provenance
through exact helper parameters and named struct fields. It keeps external,
reassigned, escaping, and unclosed fields unresolved. The implementation is
split into `test_assertions/generated_inputs.rs`; the external-oracle module
no longer owns generated-path dataflow.

The IEC inventory's repository scan uses a separately validated
`repository_source_tree_read_to_string!` contract. The extractor accepts only
canonical repository-relative literal roots and one literal extension,
rejects absolute, parent, dynamic, missing, or symlinked trees, and binds every
allowed Rust source digest. The accepted inventory records 1,571 source files.
That closure is intentionally invalidated if a scoped Rust source changes.

The case-helper tests were behavior-preserving refactors: generated paths are
constructed visibly before helper calls and `RunConfig` literals retain exact
field provenance. No production behavior changed. The IEC report logic was
extracted into pure parse/render/failure helpers, and two genuinely missing
negative tests now prove malformed-map and missing-name rejection.

## Exact assertion and execution proof

The clean builder campaign passed 15/15 selected tests in two exact
`cargo nextest` groups:

- three `trust-hir --test iec_table_coverage` tests;
- twelve `verification-cases --lib` tests.

The fifteen accepted assertion inventories contain:

- 120 extracted assertions;
- 54 direct `assert*` or `expect_err` oracles;
- zero assertion-empty tests;
- zero unresolved assertion helpers;
- exact source closure for every selected identity.

The atomic mapping transaction then:

- changed ten existing reviewed-nonmapping facts to mapped;
- registered and mapped the two new IEC test facts;
- refreshed all three previously mapped case-helper provenance records;
- updated both invariant test lists, coverage cells, and focused missing-item
  ledgers;
- left the proposal queue empty;
- validated the 27-behavior residual before one recoverable apply.

The focused Python proof, campaign, route, and transaction suite passed 9/9
locally and 9/9 on the exact metadata commit. At that same clean builder
commit, the assertion extractor suite passed 23/23, the case-helper crate
passed 17/17, and the IEC inventory target passed 3/3: 52/52 focused checks.

## Durable artifacts

The accepted machine-readable artifacts are retained under:

```text
docs/internal/testing/evidence/plc-verification-program/2026-07-29/
  p18-verification-owned-exact-behavior-11/
```

Their principal digests are:

```text
campaign.json
  sha256:f7bdf468dd10f5103732107744c8b2d85b1cf1166b253847977c7e393a3a69a0
campaign-plan.json
  sha256:cb98ba04a39b3a59a613c48fe9ac19ca1417a725be47c770f67ede89f75ad101
assertions.json
  sha256:e15e3d96d583539ba5850c45173e30d99881ed4129f6fbf9743eeae939bc944a
live-scan.json
  sha256:07decfda835d98376a31326d25eb667c82cf882d39ed84f9444e2308daa23937
pre-mapping-gap-report.json
  sha256:05296741a2cf437158bb282eed4bca614e847cc91371af0c8272d0d6b3177aa1
post-mapping-gap-report.json
  sha256:b3fb67f0117ab2c1910b767d9d9b887462ada7125f3b8a03994d9b88d995face
```

The assignments and evidence digests are also bound in
`verification/p18/review-locks/verification-owned-11-semantic-lock.json`.

## Scoped and global validation state

The complete changed-path planner remained red on 1,354 open repository-wide
Phase 18 metadata findings. The full catalog-staleness command also remains red
on the existing aggregate metadata/proof baseline; the scoped transaction
compiler, fresh live scan, exact source closures, denominator equations, and
post-mapping report all pass.

This work concerns truST verification infrastructure rather than IEC 61131-3
language behavior. It does not require an IEC decision or deviation.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
