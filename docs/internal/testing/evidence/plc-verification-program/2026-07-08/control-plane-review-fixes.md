# PLC Verification Control-Plane Review Fixes

Date: 2026-07-09
Branch: `plc-verification-program`
Scope: Review findings `CP-01` through `CP-17`

## Summary

The follow-up review returned `clear-with-edits`. This slice fixes the reported
validator and metadata gaps while keeping the stop boundary intact:

- no `plan_tests.py`
- no `gen_cases.py`
- no `prove.py`
- no `verification-cases` crate
- no product/runtime/compiler behavior changes

## Fixed Findings

- `CP-01`: replaced the validator's ad hoc case-family list with the canonical
  19 coverage dimensions and remapped `RT_SAFE_IO_WORKER_001` from `timeout` to
  `concurrency_or_cancellation`.
- `CP-02`: documented that areas with zero `test_mapping` required-spec rows
  are uninventoried and must fail closed when the planner lands.
- `CP-03`: added and enforced spec-gap class vocabulary; folded release/public
  claim gaps into `public_claim_unproven`.
- `CP-04`: high-risk `implemented`/`validated` invariants now require
  allowlisted `green` or `lock_compare` evidence that back-links the invariant.
- `CP-05`: behavior rows with `spec_gap_ref` cannot carry expected outcome,
  delta, error code, no-partial-apply, or fault-surface fields; partition keys
  are allowlisted.
- `CP-06`: `status = "spec_gap"` invariants may point `oracle.ref` at their
  gap; other statuses may not use a spec gap as oracle.
- `CP-07`: the skeleton evidence report is updated and points here for current
  post-review counts.
- `CP-08`: evidence kind, commit marker, suite/release owner, and
  kind-specific required fields are validated.
- `CP-09`: the source-build public claim now has a release invariant, spec gap,
  and linked supporting evidence.
- `CP-10`: invariants must name `spec.source_refs` or `spec_gap_refs`.
- `CP-11`: invariant filename and area directory must match the record, and
  stray nested TOML files under `verification/invariants` fail validation.
- `CP-12`: proof levels and test classes are enumerated and enforced.
- `CP-13`: `covered` and `covered_by_fuzz` cells require tests.
- `CP-14`: validator required-field lists and schema `required` arrays are
  cross-checked; selected schema enums are cross-checked against the validator
  vocabulary.
- `CP-15`: `veryquick` and `pr` suite seeds are explicitly marked placeholder
  metadata-only suites; supporting local proof uses `supporting_local`.
- `CP-16`: VM determinism/resource limits now have a dedicated spec gap and
  invariant instead of borrowing the bytecode-validator gap.
- `CP-17`: the validator is split into a package under
  `scripts/verification/metadata_validator/`; the compatibility entrypoint
  remains at `scripts/validate_verification_metadata.py`.

## Validator Proof

Command:

```sh
python3 scripts/validate_verification_metadata.py
```

Expected current result after this report is indexed:

```text
verification metadata validated: 67 records
```

Gate command:

```sh
scripts/verification_metadata_gate.sh
```

Expected current result after this report is indexed:

```text
verification metadata validated: 67 records
```

Additional cheap checks for this slice:

- `python3 -m py_compile scripts/validate_verification_metadata.py scripts/verification/metadata_validator/core.py scripts/verification/metadata_validator/constants.py`
- recursive TOML parse under `verification/**`
- recursive JSON parse under `verification/schemas/**`
- in-memory adversarial smoke checks for off-canon coverage families,
  spec-gap behavior outcome laundering, waived matrix rows without reviewed
  decisions, and invalid evidence kind/commit markers
- `git diff --check`
- checklist-row duplicate scan
- line-count guardrail for validator modules and metadata docs
