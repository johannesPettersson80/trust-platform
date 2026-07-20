# P1B Adversarial Self-Test Fixtures

Date: 2026-07-09

Implemented row:

- `VERIF-P1B-011`: adversarial self-test fixtures for planner, case, and proof
  tooling.

What changed:

- Added `scripts/verification/adversarial_selftest_tests.py`.
- Added ten fixtures covering the row's required bypass inventory:
  - assert-nothing red proof,
  - skipped case,
  - stale case-file digest,
  - missing oracle,
  - spec-gap closure while still referenced,
  - risk downgrade visibility,
  - manual safety evidence,
  - compile-error-as-red,
  - uncataloged changed test,
  - unmapped file.
- Folded two P1B-010 burn-in fixes:
  - `changed_files_from_git` now uses merge-base diffing so stale base-branch
    changes do not pollute pull-request reports.
  - the report gate's default output directory is now
    `target/gate-artifacts/verification`, matching the rest of the proof
    tooling and avoiding untracked root `gate-artifacts/` output.
- Added explicit `contents: read` workflow permissions.
- Removed a duplicate `/src/test/` marker in the report gate test-path
  classifier.

Current proof status:

- These are self-tests of the verification tooling, not product proof.
- No generated `proof_kind` evidence was added.
- No spec gap was closed.
- `VERIF-STOP-012` remains closed; no skill or agent mandate was updated.

Stop boundary:

- No runtime, VM, compiler, LSP, VS Code extension, or product behavior changed.
- No CI enforcement flip was made; the verification workflow remains
  report-only.
- No release metadata changed.

Focused validation run:

- `python3 -m unittest scripts.verification.adversarial_selftest_tests scripts.verification.report_gate_tests scripts.verification.prover_tests scripts.verification.metadata_validator.evidence_proof_tests scripts.verification.bytecode_transforms_tests`
  - Result: 84 tests passed.
- `python3 -m py_compile scripts/plan_tests.py scripts/verification/planner.py scripts/verification_report_gate.py scripts/verification/report_gate.py scripts/verification/report_gate_tests.py scripts/verification/adversarial_selftest_tests.py`
  - Result: passed.
- `python3 scripts/verification_report_gate.py --changed crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs crates/trust-runtime/tests/new_verification_report_smoke.rs --intent bugfix`
  - Result: exit `0`.
  - Wrote reports under `target/gate-artifacts/verification/`.
  - Reported `verification_metadata_gate` exit `0`.
  - Reported planner exit `4` because the synthetic new test path is unmapped
    by the bytecode/VM pilot matrix.
  - Reported the synthetic test path as uncataloged.
- `python3 scripts/validate_verification_metadata.py`
  - Expected current result after this evidence row is indexed:
    `verification metadata validated: 96 records`.
- `scripts/verification_metadata_gate.sh`
  - Expected current result after this evidence row is indexed:
    `verification metadata validated: 96 records`.
- `git diff --check`
  - Result: passed.
