# Phase 1B Case Generator Review Fixes

Date: 2026-07-09
Scope: `VERIF-P1B-005A`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## Review Findings Folded

- `GC-01`: `equals` behavior rows now require explicit `case_family`.
  Generated cases carry the label as `input.scenario`, not as the typed input
  value. The `FB_INPUT_COPY_IN_LENGTH_6` row is now `above_max`, not
  `happy_path`.
- `GC-01` / P1B-006 gate design: generated case files now include
  `generator_digest`, derived from `scripts/verification/case_generator.py` and
  `scripts/verification/metadata_validator/constants.py`.
- `GC-02`: mixed partition key sets fail validation. The only multi-key shape
  allowed in v1 is `{ min, max }`.
- `GC-03`: metadata-invalid generation exits `6`; normal generation drift,
  unknown invariant, and non-pilot scope errors remain exit `1`.
- Non-blocking cleanup folded: case IDs are now derived from partition content,
  not behavior-row ordinal, and generated inputs carry `source_partition`
  instead of `source_behavior`.
- Pre-existing cleanup folded: `git check-ignore` errors now fail durable
  evidence validation instead of being treated as "not ignored".

## Stop Boundary

Not implemented in this slice:

- no committed pilot case tables under `verification/cases/bytecode_vm/**`;
- no test-catalog case digests;
- no `prove.py`;
- no `verification-cases` crate;
- no bytecode/VM/runtime product behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 -m py_compile scripts/gen_cases.py scripts/verification/case_generator.py scripts/verification/metadata_validator/oracle_refs.py scripts/verification/metadata_validator/core.py scripts/verification/metadata_validator/constants.py scripts/validate_verification_metadata.py`
  - Result: pass.
- `python3 scripts/validate_verification_metadata.py`
  - Result before indexing this evidence record:
    `verification metadata validated: 74 records`.
- `python3 scripts/validate_verification_metadata.py`
  - Result after indexing this evidence record:
    `verification metadata validated: 75 records`.
- `scripts/verification_metadata_gate.sh`
  - Result: `verification metadata validated: 75 records`.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --format json`
  - Result: `equals` rows emit `input.scenario` and `source_partition`; no
    typed `assignment` value is fabricated for scenario labels.
- `python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001`
  - Result: the FB copy-in scenario emits `family = "above_max"` and
    `input.scenario = "FB_INPUT_COPY_IN_LENGTH_6"`.
- In-memory adversarial validator probes:
  - `{ above = 5, equals = "MIXED" }` fails as an unsupported mixed partition.
  - An `equals` partition without `case_family` fails.
  - A non-`equals` row with `case_family` fails.
- Metadata-invalid exit-code probe:
  - Imported `verification.case_generator` with the same `scripts/` module path
    the command shim uses, patched `load_metadata` to raise
    `MetadataInvalidError`, and called `main(["--invariant",
    "VM_SEAM_SUBRANGE_001"])`.
  - Result: `exit:6`, proving metadata-invalid generation uses the declared
    exit code.
- Output-shape probes:
  - Declared-type `equals` cases carry `input.scenario` and no typed
    `assignment` value.
  - The `FB_INPUT_COPY_IN_LENGTH_6` string-bound case is `above_max`, carries
    `input.scenario`, omits `string_value`, and the case file has
    `generator_digest`.

## Acceptance Notes

`VERIF-P1B-006` may now commit pilot case tables with both invariant
`source_digest` and generator `generator_digest`. Its gate must regenerate
through the derived default path so a caller-supplied `--check` path cannot hide
case-table drift.
