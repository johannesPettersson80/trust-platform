# P1B-006 Review Fixes

Date: 2026-07-09
Branch: plc-verification-program
Scope: VERIF-P1B-006A only.

## Review Findings Folded

- `CT-01`: case-file `expect` validation now resolves `oracle_ref` through the
  normal oracle rules and requires the expected payload to match an
  oracle-backed behavior row on the named invariant.
- `CT-02`: `scripts/verification_metadata_gate.sh` now runs derived-path
  `gen_cases.py --check` for every catalog row that carries `case_file`.
- `CT-03`: wrong-type and malformed generated cases now use
  `input.shape_descriptor` instead of the invariant typed input field.
- `CT-04`: case-file required fields are read from
  `SCHEMA_REQUIRED_FIELDS["case-file.schema.json"]` instead of a copied list.
- Import cleanup: `case_generator.py` uses package-relative imports.

## Stop Boundary

This remains metadata/tooling only. It does not add `crates/verification-cases`,
`prove.py`, runnable tests, product runtime or VM behavior changes, CI
enforcement, or skill updates.

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

python3 - <<'PY'
from scripts.verification.metadata_validator.core import Validator

v = Validator()
v.load_records()
case = v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"]
case["case_file_digest"] = "sha256:bad"
v.validate()
messages = [failure.message for failure in v.failures]
hit = any("case_file_digest mismatch" in message for message in messages)
print("catalog digest mismatch probe:", "PASS" if hit else "FAIL")
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

python3 - <<'PY'
from copy import deepcopy
from scripts.verification.metadata_validator.core import Validator

v = Validator()
v.load_records()
record = deepcopy(v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"])
record["case_file_digest"] = v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"]["case_file_digest"]
v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"] = record
case_file = record["case_file"]
text = open(case_file).read()
tampered = text.replace(
    'state = "blocked"\nspec_gap_ref = "SPEC_GAP_VM_VALUE_SEMANTICS_001"',
    'expect = { outcome = "reject", oracle_ref = "SPEC_RUNTIME_SEMANTICS_001#fabricated" }',
    1,
)
open(case_file, "w").write(tampered)
try:
    v = Validator()
    v.load_records()
    # Keep this probe focused on expected-payload validation rather than the
    # catalog digest guard.
    import hashlib
    actual = "sha256:" + hashlib.sha256(open(case_file, "rb").read()).hexdigest()
    v.tests["TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001"]["case_file_digest"] = actual
    v.validate()
    messages = [failure.message for failure in v.failures]
    hit = any("expect does not match an oracle-backed behavior row" in message for message in messages)
    print("fabricated expect probe:", "PASS" if hit else "FAIL")
    raise SystemExit(0 if hit else 1)
finally:
    open(case_file, "w").write(text)
PY

for file in verification/cases/bytecode_vm/*.toml; do
  if rg -n 'wrong_type = .*}, (value|string_value|assignment) =' "$file"; then
    exit 1
  fi
done

# Co-drift probe: tamper a case file and update its catalog digest. Metadata
# validation alone should pass, but the gate must fail on derived-path
# regeneration drift.

git diff --check
```

## Observed Local Result

All commands above passed locally. `scripts/verification_metadata_gate.sh`
validated `80 records` and ran the three derived-path case checks. The co-drift
probe produced `case file drift` from `gen_cases.py --check`.
