# Phase 14 Governance Closeout

Date: 2026-07-19

Implementation checkpoint: `7baa48646ce85332c72ca9a9fdb6f66a90f6345a`

Report rebind checkpoint: `1d2f0ff9`

## Scope

This closeout records the governance contract implemented by
`verification/governance.toml`, the append-only retirement registry in
`verification/retirements.toml`, and the existing report gate integration. It
creates no product proof and changes no suite or approved proof producer.

The contract defines:

- closed owner and alias rules for all eleven verification areas;
- explicit display-only suite include/exclude semantics with no command,
  execution, or proof inheritance;
- a 90-day active-metadata review limit;
- 30-day grace periods for unknown ignored tests, missing eligible oracles,
  and undispositioned public claims;
- a before-merge deadline for safety-relevant mutation survivors;
- area coverage-dimension templates, with the bytecode dimensions bound to
  the existing matrix and all other dimensions requiring per-invariant review;
- same-diff invariant and catalog updates for product changes, and spec-source
  plus invariant updates for public-claim changes;
- monthly ignored-test, release-time hardware/security, and quarterly
  mutation/fuzz review cadence; and
- append-only retirement tombstones that retain the original invariant or
  evidence record and bind retirement evidence.

## Validation

Focused tests at the implementation checkpoint:

```text
python3 -m unittest \
  scripts.verification.governance_tests \
  scripts.verification.report_gate_tests \
  scripts.verification.phase13_release_tests

Ran 26 tests: OK
```

Current-state checks after the report rebind:

```text
python3 -m scripts.verification.governance --today 2026-07-19
verification governance: PASS (0 changed paths)

python3 scripts/validate_verification_metadata.py
verification metadata validated: 840 records
```

The live metadata join contains zero ignored tests classified `unknown` and
zero high-risk invariants without an active, oracle-eligible, non-public-claim
source. Focused adversarial fixtures prove stale metadata, overdue unknown
ignores, overdue high-risk missing oracles, overdue cadences, invalid owner
rules, suite proof inheritance, changed-product omissions, and invalid
retirements fail closed.

The Phase 12 and Phase 13 reports were regenerated independently from the
clean implementation checkpoint after their input closures were corrected to
include every Python validator module they execute. Both validate at rest.
Their JSON SHA-256 values are:

- Phase 12: `1e74807c0c026af4d43e991a61f000f7f2049568369bd9d65349c2c575f276b8`
- Phase 13: `0a6279593a35157137da4a7f162542a513024813182a21f3eb77683a8e6090bc`

## Boundaries

This is metadata governance only. It does not add product tests, close a spec
gap, promote an invariant, create proof evidence, alter CI enforcement, change
a suite, or authorize a proof producer. Forward, reverse, and orphan
traceability reports remain open under `VERIF-P6-008` through
`VERIF-P6-010`.
