# Phase 16 Test-Catalog Denominator Closure

Date: 2026-07-18

## Scope

This record closes the last open denominator in `VERIF-P16-006`. It does not
add or modify a product test, infer a semantic mapping, change a suite, or flip
CI enforcement.

## Clean-Source Report

- Implementation commit: `275adecd1c3a4308db7af37af1072a580e328e52`
- Report timestamp: `2026-07-18T18:00:00+02:00`
- Platform: `linux-aarch64`
- Report JSON SHA-256:
  `74c8515872413f281bb836b00219e3ba6abe4eac6d17d10760b94da049561c25`
- Denominator review SHA-256:
  `sha256:42def5a36e54b5ea791c0d268fe2e4f969e32bc52f389c0d0fe88615e77256f3`

The committed v2 Markdown and its ignored machine JSON report:

- 4,023 live scanner facts;
- 241 exact catalog mappings;
- 3,782 exact reviewed-nonmapping dispositions;
- 0 unreviewed facts;
- 23 ignored-register-owned facts;
- 8 fuzz-program-owned facts;
- 59 gate-inventory-owned facts; and
- 3,692 facts with no reviewed specification or invariant binding.

The raw 3,782 non-catalog identities remain rendered in the report. Their
reviewed disposition retires mapping debt without deleting the tests or
claiming an invariant, specification, oracle, expected result, passing result,
or assertion adequacy.

## Commands

```text
python3 -m unittest scripts.verification.test_catalog_denominator_tests scripts.verification.test_catalog_debt_tests
python3 scripts/validate_verification_metadata.py
python3 scripts/report_unmapped_test_debt.py --json-out target/gate-artifacts/verification/unmapped-test-debt.json --markdown-out docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md --timestamp 2026-07-18T18:00:00+02:00
python3 scripts/validate_unmapped_test_debt_report.py --json target/gate-artifacts/verification/unmapped-test-debt.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md
scripts/verification_metadata_gate.sh
git diff --check
```

The implementation commit was clean before report generation. The report
generator and at-rest validator each re-scanned the live source population and
recomputed the exact catalog, ignored-register, schema, input-digest, and
Markdown bindings.

## Boundaries

- `VERIF-P16-007` and `VERIF-P16-008` remain open.
- CI remains report-only.
- No suite or `approved_proof_producers` entry changed.
- No product, runtime, compiler, editor, test-source, skill, or agent file
  changed.
- Reviewed nonmapping creates no proof and closes no specification gap.
