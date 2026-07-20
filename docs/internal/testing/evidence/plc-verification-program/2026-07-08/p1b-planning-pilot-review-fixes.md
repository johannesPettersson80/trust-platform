# Phase 1B Planning Pilot Review Fixes

Date: 2026-07-09
Scope: `VERIF-P1B-003B`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata/tooling changes

## Review Findings Folded

- `PB-01`: Added taxonomy-to-metadata drift checking. The validator parses
  `test-taxonomy.md` test-class and coverage-dimension lists and compares them
  with `TEST_CLASSES` and `CASE_FAMILIES`; `verification_metadata_gate.sh`
  exercises this through the metadata validator.
- `PB-02`: Added planner risk reporting for mapped, unmapped,
  uninventoried, and unknown-area inputs. Planner output now prints waivers,
  and baseline risk-change comparison has an explicit `--baseline` path.
- `PB-03`: `plan_tests.py` now runs the metadata validator before planning and
  exits separately on invalid metadata.
- `PB-04`: Usage errors now exit `5`, distinct from semantic missing-tests
  exit `2`.
- `PB-05`: Planner missing-class computation is per area and only runnable test
  statuses count as coverage. The validator rejects runnable catalog rows whose
  paths do not exist.
- `PB-06`: `crates/trust-runtime-core/src/error.rs` is now mapped into the
  bytecode/VM pilot area.
- `PB-07`: The validator rejects orphan open spec gaps, and the planner also
  reads open spec gaps by resolved area so area-level gaps still block even if a
  required-spec row is self-attested.
- `PB-08`: Evidence/traceability details were split from `metadata-model.md`
  into `metadata-evidence-traceability.md`; new validator logic was added in
  dedicated modules rather than appended into `core.py`.

## Stop Boundary

Not implemented in this slice:

- no `VERIF-P1B-004` behavior rows;
- no case generation;
- no `prove.py`;
- no `verification-cases` crate;
- no product runtime/VM behavior changes;
- no CI enforcement or skill update.

## Validation

- `python3 -m py_compile scripts/plan_tests.py scripts/verification/planner.py scripts/verification/metadata_validator/core.py scripts/verification/metadata_validator/constants.py scripts/verification/metadata_validator/taxonomy.py scripts/verification/metadata_validator/integrity.py scripts/validate_verification_metadata.py`
  - Result: pass.
- `python3 scripts/validate_verification_metadata.py`
  - Result before indexing this evidence record: `verification metadata validated: 70 records`.
- `python3 scripts/validate_verification_metadata.py && scripts/verification_metadata_gate.sh`
  - Result after indexing this evidence record: `verification metadata validated: 71 records` for both commands.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs`
  - Result: `Verdict: spec_gap`, `Exit code: 3`, with bytecode/VM risk reporting and no expected behavior text.
- `python3 scripts/plan_tests.py --intent docs --changed verification/invariants/bytecode_vm/VM_SEAM_VALID_001.toml --format json`
  - Result: valid JSON, `required_test_classes = ["metadata_validation"]`, no case families, `exit_code = 3`.
- `python3 scripts/plan_tests.py --intent bugfix --changed README.md`
  - Result: `Verdict: unmapped`, `Exit code: 4`, with highest-risk note.
- `python3 scripts/plan_tests.py --intent refactor --area runtime_safety`
  - Result: `Verdict: spec_gap`, `Exit code: 3`, `Uninventoried areas: runtime_safety`.
- `python3 scripts/plan_tests.py --intent bugfix --changed crates/trust-runtime-core/src/error.rs`
  - Result: `Verdict: spec_gap`, `Exit code: 3`, mapped to `bytecode_vm`.
- `python3 scripts/plan_tests.py --intent bogus --changed x.rs`
  - Result: argparse usage error, process exit `5`.
- `python3 scripts/plan_tests.py --intent refactor --area typo_area`
  - Result: `Verdict: spec_gap`, `Exit code: 3`, `Unknown areas: typo_area`.

## Line-Count Notes

- `metadata-model.md` was reduced below the 1k-line threshold by splitting
  evidence/traceability details into `metadata-evidence-traceability.md`.
- New metadata validator checks live in `taxonomy.py` and `integrity.py`.
