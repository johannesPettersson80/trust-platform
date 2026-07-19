# Phase 8 Exhaustive Runtime-Anomaly Denominator Closure

Date: 2026-07-18

Source commit: `5395f5d969d7f5828dc2e7e3701f6d7bc7f69a56`

## Scope

This slice closes only `VERIF-P8-002`. It reviews the complete live Rust-test
denominator used by the Phase 8 audit and records exactly one disposition for
every stable discovery ID. It does not execute a fault, create proof or
invariant coverage, close a specification gap, add a fault interface, change
product behavior, or change CI enforcement. `VERIF-P8-005` and
`VERIF-P8-006` remain open.

## Reviewed Partition

| Measure | Count |
| --- | ---: |
| Live Rust test facts | 3,220 |
| Explicit anomaly mappings | 133 |
| Reviewed nonmappings | 3,087 |
| Unreviewed facts | 0 |

Reviewed nonmapping rationales:

| Rationale | Count |
| --- | ---: |
| Outside runtime-safety scope | 1,298 |
| No taxonomy stimulus or response | 719 |
| Supporting internal contract only | 919 |
| Different safety domain | 151 |

The partition is disjoint and exhaustive. Mapped rows bind an existing mapping
ID. Nonmapping rows use a closed rationale code. Every row also binds the live
discovery ID, source kind, path, and test name; line numbers are deliberately
not identities. A new, deleted, duplicated, renamed, moved, or rebound test
causes the live join to fail until the ledger is reviewed again.

The denominator review digest is
`sha256:fb44c8af3c1727e63b99248acfabb8d0b42ffde21059ba25d8d041e241419cb3`.

## Phase 8 Report

The clean-source Phase 8 report was generated with:

```text
python3 scripts/report_runtime_anomaly_audit.py --json-out target/gate-artifacts/verification/runtime-anomaly-audit.json --markdown-out docs/internal/testing/evidence/plc-verification-program/2026-07-11/p8-runtime-anomaly-audit.md --timestamp 2026-07-18T09:53:26+02:00
python3 scripts/validate_runtime_anomaly_audit_report.py --json target/gate-artifacts/verification/runtime-anomaly-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-11/p8-runtime-anomaly-audit.md
```

The report was generated and validated in the isolated clean
`trust-builder` checkout. The generated JSON SHA-256 is
`2d3daaeb511608c8ea56c5febf76b40857d4d84f66b536fb4f9acedbd85ef339`.
The report input digest is
`sha256:cc02b0a06deb0c146685eebd2f1cdebfc62a55373c6a56fdd553729f4f8d198b`.
It reports 19 classes, 133 associations, 123 effectively runnable direct
mappings, one ignored or conditional mapping, and zero class-level test gaps.
The zero gap count is association posture only, not adequacy or proof.

## Validation

The implementation commit passed:

```text
python3 -m unittest scripts.verification.runtime_anomaly_denominator_tests scripts.verification.runtime_anomaly_mapping_tests scripts.verification.runtime_anomaly_contract_tests scripts.verification.runtime_anomaly_report_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
git diff --check
```

The focused run completed 60 tests successfully. Static metadata validated 734
records before this evidence row was indexed. The metadata gate, including all
generated case-table checks, passed. No Rust, Node, product, or workflow source
changed in this slice, so no broad compiled gate was rerun.
