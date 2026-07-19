# Phase 1A Specification-Source Audit Slice Closure Validation

Date: 2026-07-13
Branch: `plc-verification-program`
Implementation checkpoint: `744d4df631b1ac88da20cd62703c8ff882e76113`
Mutation-evidence checkpoint: `fdd67bb3ebd1d81413d62639b523f29a6d2e1542`
Deterministic-report checkpoint: `d4bcdddfd2abb12abac6c0fd876dc36d396a2565`
Validated evidence checkpoint: `d16ee31dd4a81674e35dd483cd88091f3316fc31`
Proof posture: report-only inventory evidence; this record adds no product proof

## Closed Scope

This closure records the completed specification-source audit slice:

- `VERIF-P1A-002`: mechanical specification-document and rendered-public-prose
  denominator;
- `VERIF-P1A-004`: ordered 21-topic obvious-missing-specification ledger;
- `VERIF-P1A-007`: reviewed metadata for the unavailable external IEC source,
  without reading or provenance-binding local standard bytes;
- `VERIF-P1A-008`: all 19 bytecode/VM required-specification rows resolve to
  either an active source or an actionable open specification gap;
- `VERIF-P1A-009`: mapped test rows cannot omit both an oracle and a
  specification-gap reference; and
- `VERIF-P6A-005`: specification-source scanner bypass fixtures exercise the
  production source-contract catchers.

The slice does not close Phase 1A. `VERIF-P1A-003` remains open because 375
discovered documents still have the mechanical `unreviewed_candidate`
disposition. `VERIF-P1A-006` remains open because checklist-row staleness and
references to removed product behavior have not received exhaustive semantic
review. `VERIF-P4A-005` remains open because 14,098 discovered public-prose
blocks have not received semantic public-claim dispositions.

## Measured Audit Denominator

The committed report at
`docs/internal/testing/evidence/plc-verification-program/2026-07-13/p1a-spec-source-audit.md`
records:

- 392 discovered documents;
- 20 registered sources: 19 tracked-file bindings and one external reference;
- 375 unreviewed documents;
- 19 required topics: 10 source-mapped, 9 gap-open, 0 broken;
- 21 obvious specification topics: 2 source-present, 8 gap, 8 partial, and 3
  unrepresented;
- 178 public surfaces;
- 14,102 public-prose blocks;
- 4 registered public claims, all four bound by exact reviewed text;
- 14,098 unreviewed public-prose blocks;
- 0 scanner diagnostics;
- 1 registered source review due; and
- 0 blocking findings and 118 warning findings.

The report JSON SHA-256 is
`c73f938e127fd1fb56a57ef810ed21cc3484634a4a755da4a473ba46dd5f199d`.
The Markdown SHA-256 is
`234c2a6951a20a9b6f9836c0702c6e7b6ecdce2126444113788eb7f61290afba`.

No title, path, heading, prose similarity, or lexical candidate creates a source,
requirement, claim, invariant, test, or proof mapping. Authority, ownership,
area, and oracle eligibility come only from reviewed metadata.

## Tests-First Corrections

The implementation was exercised through production metadata, report,
source-revision, schema, Markdown, and tooling-self-test paths. Review fixes
hardened hostile types, schema-enum drift, external-source isolation,
surface-reference containment, public-claim exact binding, intermediate gap
states, and the live open-row boundaries.

Pristine report generation exposed one real verification-tooling defect:
Markdown boundary rows followed insertion order before canonical JSON reload and
could therefore differ at rest. A regression test first reproduced the mismatch;
commit `d4bcdddfd2abb12abac6c0fd876dc36d396a2565` makes rendering use the
reviewed canonical boundary order. This was a tooling defect, not a product
runtime defect.

## Bytecode Mutation Refresh

The bytecode-validator mutation shard was rerun on `trust-builder` against clean
commit `744d4df631b1ac88da20cd62703c8ff882e76113` using
`cargo-mutants 27.0.0`.

Result:

- 2 total;
- 2 caught;
- 0 survived;
- 0 unviable;
- 0 timeout; and
- 0 infrastructure error.

The refreshed machine report SHA-256 is
`14f8c0651772b976105d113d49261618e6b9ac8d6407fb872042f02ef346a40d`.
Its bound case-file digest is
`sha256:2fc357301eeca9bdabfd6d56eacafd6c6a7643cac1e67628e75968f21848046e`.
Case IDs, selectors, commands, and outcomes were unchanged. The blocked case IDs
remain associations only and were not claimed as executed proof.

## Report Regeneration And Revalidation

Fourteen affected report pairs, including the new source audit, were generated
from pristine worktrees at
`d4bcdddfd2abb12abac6c0fd876dc36d396a2565` with timestamp
`2026-07-13T09:31:00+02:00`. The existing P3 ignored-test report did not require
regeneration because its explicit closure was unchanged.

All 15 installed report pairs passed their production at-rest validators after
the evidence index was rebound in
`d16ee31dd4a81674e35dd483cd88091f3316fc31`. The 33-case verification-tooling
fixture report also regenerated and validated successfully.

## Local Validation

At the clean evidence checkpoint:

- `python3 scripts/run_verification_focused_tests.py`: 763/763 passed in
  644.949 seconds.
- `scripts/verification_metadata_gate.sh`: 352 metadata records before this
  closure row was indexed.
- `python3 scripts/check_verification_tooling_selftests.py`: 33/33 fixtures
  passed.
- `python3 scripts/check_ignored_test_staleness.py`: 88 discovered, 88
  registered, 63 unknown, 0 catalog-mapped.
- `python3 scripts/check_test_catalog_staleness.py`: 7 committed catalog records
  against 3,821 scanner facts.
- `python3 scripts/check_vscode_test_registration.py`: 456 facts, 38 files, 38
  registrations.
- `python3 scripts/validate_test_refactor_proposals.py`: 1 proposal, 0 redirects,
  7 catalog records, 3,821 scanner facts.
- All four `gen_cases.py --check` invocations passed.
- All 15 report validators passed.
- `git diff --check` passed and the worktree was clean.

After indexing this closure row, both metadata entrypoints report 353 records.

## Remote Builder Validation

The retained validation checkout
`$HOME/projects/trust-platform-spec-audit-validation-744d4df6` was clean at
`d16ee31dd4a81674e35dd483cd88091f3316fc31`.

The following sequence passed on `trust-builder`:

```text
cd "$HOME/projects/trust-platform-spec-audit-validation-744d4df6"
export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate"
export TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp"
just fmt
just clippy
just verification-veryquick
just test-all
```

`just verification-veryquick` included the 763-test focused Python suite and its
selected Rust and conformance checks. `just test-all` completed successfully.
The generated target expanded to 71 GiB and left only 89 MiB free after the
successful run. After confirming no compiler processes remained, only the two
generated target/cache directories were removed; the builder returned to 71 GiB
free under `/home/johannes` and 3.2 GiB under `/tmp`. The retained source
checkout remained clean.

## Final Posture And Boundaries

Before this closure evidence row, the implementation board is 173/244 checked.
The program has 34 specification gaps, 33 open; 52 invariants, 51 at S0 and
`IEC_TIMER_001` at G2. No invariant is marked validated.

This slice:

- creates no product proof;
- closes no specification gap;
- promotes no invariant;
- changes no runtime, compiler, LSP, IDE, HMI, protocol, or PLCopen behavior;
- changes no product test;
- changes no suite, workflow, approved proof producer, CI enforcement, skill, or
  agent instruction; and
- leaves `VERIF-STOP-012`, `VERIF-STOP-014`, `VERIF-P1B-012`, and
  `VERIF-P1B-014` open.

The 118 audit warnings and the unreviewed document/public-prose populations are
visible debt, not accepted specifications or claims.

This closes only the already-started audit evidence bind. The next development
slice is the E2 timer/restart product vertical: written decisions only where
required, product-facing traces and tests first, then red, minimal product fix,
green, and broad `trust-builder` evidence. No further control-plane expansion is
authorized unless an existing mandatory gate is red and requires a minimal
tooling correction.
