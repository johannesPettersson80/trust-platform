# P1B-006 Committed Pilot Case Tables

Date: 2026-07-09
Branch: plc-verification-program
Scope: VERIF-P1B-006 only.

## What Changed

- Added committed bytecode/VM pilot case tables:
  - `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`
  - `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`
  - `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`
- Added planned catalog rows that pin each case table by SHA-256 digest.
- Added case-file validation for cataloged case tables:
  - table shape and case-family vocabulary,
  - invariant source digest,
  - generator-source digest,
  - blocked-case `spec_gap_ref`,
  - expected-outcome rows only when an oracle-backed behavior row exists.
- Updated the case-file schema and metadata model so `source_digest` is required.

## Stop Boundary

This slice does not add `crates/verification-cases`, `prove.py`, runnable
verification tests, product runtime or VM behavior changes, CI enforcement, or
skill updates. The catalog rows are `status = "planned"` and do not count as
behavior proof.

## Expected Current Result

The three generated case tables are intentionally blocked planning artifacts.
They keep the value-semantics spec gap visible and do not choose truncate,
reject, fault, or conversion behavior for the seeded VM value cases.

## Validation To Reproduce

```sh
python3 -m py_compile \
  scripts/gen_cases.py \
  scripts/verification/case_generator.py \
  scripts/verification/case_digests.py \
  scripts/verification/metadata_validator/case_files.py \
  scripts/verification/metadata_validator/core.py \
  scripts/verification/metadata_validator/constants.py \
  scripts/validate_verification_metadata.py

python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh

python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check

python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 \
  | if rg -n 'expect|outcome|error_code|delta'; then exit 1; else exit 0; fi

python3 scripts/plan_tests.py \
  --intent bugfix \
  --changed crates/trust-runtime-core/src/value/types.rs

python3 - <<'PY'
from scripts.verification.metadata_validator.core import Validator

v = Validator()
v.load_records()
record = v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"]
record["case_file_digest"] = "sha256:bad"
v.validate()
messages = [failure.message for failure in v.failures]
hit = any("case_file_digest mismatch" in message for message in messages)
print("digest mismatch probe:", "PASS" if hit else "FAIL")
raise SystemExit(0 if hit else 1)
PY

python3 - <<'PY'
from scripts.verification.metadata_validator import case_files
from scripts.verification.metadata_validator.core import Validator

case_files.current_generator_digest = lambda: "sha256:bad"
v = Validator()
v.load_records()
v.validate()
messages = [failure.message for failure in v.failures]
hit = any("case_file generator_digest mismatch" in message for message in messages)
print("generator digest mismatch probe:", "PASS" if hit else "FAIL")
raise SystemExit(0 if hit else 1)
PY

git diff --check
```

The planner command is expected to exit `3` while
`SPEC_GAP_VM_VALUE_SEMANTICS_001` remains open. That is the protective result:
committed case tables must not unblock implementation before the spec decision.

## Observed Local Result

- `python3 -m py_compile ...`: pass.
- `python3 scripts/validate_verification_metadata.py`: `verification metadata validated: 80 records`.
- `scripts/verification_metadata_gate.sh`: `verification metadata validated: 80 records`.
- All three default-path `gen_cases.py --check` commands: pass.
- Blocked pilot case output grep: `no expected outcome fields in blocked pilot cases`.
- Planner probe for `crates/trust-runtime-core/src/value/types.rs`: exit `3`, listing the bytecode/VM spec gaps.
- In-memory catalog `case_file_digest` mismatch probe: pass.
- In-memory generator-source `generator_digest` mismatch probe: pass.
- `git diff --check`: pass.
